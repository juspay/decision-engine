import org.codehaus.groovy.control.CompilerConfiguration
import org.codehaus.groovy.control.customizers.SecureASTCustomizer
import org.kohsuke.groovy.sandbox.SandboxTransformer
import org.kohsuke.groovy.sandbox.GroovyValueFilter
import com.sun.net.httpserver.HttpServer
import java.lang.reflect.Array
import groovy.json.JsonSlurper
import groovy.json.JsonOutput
import java.lang.Exception
import groovy.lang.GroovyRuntimeException
import java.lang.IndexOutOfBoundsException.*
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import sun.misc.Signal
import sun.misc.SignalHandler

class App {
    static void main(String[] args) {
        def dslBuilderService = new DslBuilderService()
        def jsonParser        = new JsonSlurper()
        def server            = HttpServer.create(new InetSocketAddress(8085), 0)
        
        ExecutorService threadPool = Executors.newCachedThreadPool()
        server.setExecutor(threadPool)
        
        println("Starting Groovy Runner:")
        server.createContext("/health", {
            http ->
                println "HEALTH"
                http.responseHeaders.add("Content-type", "application/json")
                http.sendResponseHeaders(200, 0)
                http.responseBody.withWriter({ out ->
                    return out.write(JsonOutput.toJson(true))
                })
        })

        server.createContext("/evaluate-script", {
            http ->
                http.responseHeaders.add("Content-type", "application/json")
                http.responseBody.withWriter({ out ->
                  try {
                    def log = new Log()
                    def body = jsonParser.parseText(http.requestBody.getText("utf8"))

                    def orderInfo   = body.orderInfo
                    def txnInfo     = body.txnInfo
                    def paymentInfo = body.paymentInfo
                    def merchantId  = body.merchantId
                    def script      = body.script
                    def context = new Context(orderInfo, txnInfo, paymentInfo)

                    println("Context: ${context.dump()}")

                    try {
                        def closure = dslBuilderService.evaluateScript(log, "gatewayPriorityLogic", merchantId, script)
                        closure.delegate = context
                        closure()
                    } catch (IndexOutOfBoundsException e) {
                        http.sendResponseHeaders(400, 0)
                        log.error("IndexOutOfBoundsException while trying to execute custom gateway priority logic for order ${orderInfo?.orderId}", e)
                        println("IndexOutOfBoundsException while trying to execute custom gateway priority logic for order ${orderInfo?.orderId} : ${e}")
                        return out.write(JsonOutput.toJson(
                            [ error  : true
                            , error_message : "CODE_TOO_LARGE"
                            , user_message  : "Script cannot be executed due to huge size of method." 
                            , log : log.log
                            ]
                        ))
                    } catch (GroovyRuntimeException e) {
                        http.sendResponseHeaders(400, 0)
                        log.error("GroovyRuntimeException while trying to execute custom gateway priority logic for order ${orderInfo?.orderId}", e)
                        println("GroovyRuntimeException while trying to execute custom gateway priority logic for order ${orderInfo?.orderId} : ${e}")
                        return out.write(JsonOutput.toJson(
                            [ error  : true
                            , error_message : "COMPILATION_ERROR"
                            , user_message  : "Script cannot be executed due to compilation issues." 
                            , log : log.log
                            ]
                        ))
                    } catch (Exception e) {
                        http.sendResponseHeaders(400, 0)
                        log.error("Exception while trying to execute custom gateway priority logic for order ${orderInfo?.orderId}" , e)
                        println("Exception while trying to execute custom gateway priority logic for order ${orderInfo?.orderId} : ${e}")
                        return out.write(JsonOutput.toJson(
                            [ error  : true
                            , error_message : "UNHANDLED_EXCEPTION"
                            , user_message  : "Script cannot be executed due to Exception" 
                            , log : log.log
                            ]
                        ))
                    }

                    http.sendResponseHeaders(200, 0)
                    return out.write(JsonOutput.toJson(
                        [ ok    : true
                        , result: context.gatewayPriorityLogicOutput
                        , log   : log.log
                        ]
                    ))
                  } catch (Exception e) {
                        http.sendResponseHeaders(500, 0)
                        log.error("Exception while trying to execute custom gateway priority logic for order ${orderInfo?.orderId}" , e)
                        println("Exception while trying to execute custom gateway priority logic for order ${orderInfo?.orderId} : ${e}")
                        return out.write(JsonOutput.toJson(
                            [ error  : true
                            , error_message : "UNEXPECTED_EXCEPTION"
                            , user_message  : "Script execution failed due to Exception ${e}" 
                            , log : log.log
                            ]
                        ))
                    }

                })
        })

        server.createContext("/", {
            http ->
                http.responseHeaders.add("Content-type", "application/json")
                http.sendResponseHeaders(200, 0)
                http.responseBody.withWriter({ out ->
                    // Body expected to be
                    // -- Actually script execution context
                    // { orderInfo  -- filtered orderInfo https://bitbucket.org/juspay/graphh/src/226fd58296575ba69976f82da411ece837587c8d/grails-app/services/juspay/MerchantGatewayPriorityService.groovy?at=master#lines-98A
                    // , txnInfo    -- filtered txn info
                    // , paymentnfo -- filtered payment info

                    // , merchantId  :: String -- merchantId. Not in script execution context, but required as part of key in cache
                    // , script     :: String -- merchant's script to execute
                    // }


                    def log = new Log()
                    def body = jsonParser.parseText(http.requestBody.getText("utf8"))

                    def orderInfo   = body.orderInfo
                    def txnInfo     = body.txnInfo
                    def paymentInfo = body.paymentInfo
                    def merchantId  = body.merchantId
                    def script      = body.script
                    def context = new Context(orderInfo, txnInfo, paymentInfo)

                    log.info("Context: " + context.dump())

                    try {
                        def closure = dslBuilderService.evaluateScript(log, "gatewayPriorityLogic", merchantId, script)
                        closure.delegate = context
                        closure()
                    }
                    catch (RuntimeException e) {
                        log.error("Exception while trying to execute custom gateway priority logic for order", e)
                        return out.write(JsonOutput.toJson(
                            [ ok    : false
                            , log   : log.log
                            ]
                        ))
                    }
                    // log.info("Dynamic gateway priorities: " + context?.gatewayPriorityLogicOutput?.asStringifyJson())
                    return out.write(JsonOutput.toJson(
                        [ ok    : true
                        , result: context.gatewayPriorityLogicOutput
                        , log   : log.log
                        ]
                    ))

                })
        })

        registerSignalHandlers(server, threadPool)

        server.start()
    }

    static void registerSignalHandlers(HttpServer server, ExecutorService threadPool) {
        println("Registering signal handlers...")
        Signal.handle(new Signal("INT"), new SignalHandler() {
            @Override
            void handle(Signal sig) {
                println("SIGINT received, initiating shutdown...")
                shutdown(server, threadPool)
            }
        })

        Signal.handle(new Signal("TERM"), new SignalHandler() {
            @Override
            void handle(Signal sig) {
                println("SIGTERM received, initiating shutdown...")
                shutdown(server, threadPool)
            }
        })
    }

    static void shutdown(HttpServer server, ExecutorService threadPool) {
        println("Shutdown initiated...")

        // Delay the server shutdown
        try {
            def sleepTimeInSeconds = System.getenv("DELAY_SHUTDOWN_SLEEP_TIME_SECONDS")?.toInteger() ?: 12
            println("Delaying shutdown for ${sleepTimeInSeconds} seconds...")
            TimeUnit.SECONDS.sleep(sleepTimeInSeconds)
        } catch (InterruptedException e) {
            println("Interrupted during delay, proceeding with shutdown...")
        }

        // Now stop accepting new connections but allow existing ones to complete
        server.stop(0)  // Stop immediately after the delay

        // Gracefully shutdown the thread pool
        threadPool.shutdown()
        try {
            // Wait for active tasks to finish
            def terminationWaitTime = System.getenv("THREAD_POOL_TERMINATION_WAIT_TIME_SECONDS")?.toInteger() ?: 30
            if (!threadPool.awaitTermination(terminationWaitTime, TimeUnit.SECONDS)) {
                println("Forcing shutdown as tasks are taking too long...")
                threadPool.shutdownNow()  // Force shutdown if tasks take too long
            }
        } catch (InterruptedException e) {
            threadPool.shutdownNow()
        }

        println("Server shutdown complete.")
    }
}
//----------------------------------------------------------------------------------------------------

// https://bitbucket.org/juspay/graphh/src/287712eee2558500e507f45a7451e24f1bb898d2/grails-app/services/juspay/MerchantGatewayPriorityService.groovy?at=master#MerchantGatewayPriorityService.groovy-125
// In this version gatewayPriority represented as list of String as we intent to serialize it later, on haskell side
class Context {
    def order
    def txn
    def payment
    def currentTimeMillis

    GatewayPriorityLogicOutput gatewayPriorityLogicOutput;
    private Context(def order, def txn, def payment) {
        this.order = order
        this.txn = txn
        this.payment = payment
        this.currentTimeMillis = System.currentTimeMillis()
        this.gatewayPriorityLogicOutput   = new GatewayPriorityLogicOutput([],false, [:])
    }
    def setGatewayPriority(List<String> gatewayPriority) {
        if(!this.gatewayPriorityLogicOutput){
            this.gatewayPriorityLogicOutput   = new GatewayPriorityLogicOutput([],false, [:])
        }
        this.gatewayPriorityLogicOutput.gatewayPriority = gatewayPriority
        this.gatewayPriorityLogicOutput.isEnforcement   = false
    }
    def enforceGatewayPriority(List<String> gatewayPriority) {
        if(!this.gatewayPriorityLogicOutput){
            this.gatewayPriorityLogicOutput   = new GatewayPriorityLogicOutput([],false, [:])
        }
        if(gatewayPriority!=null){
            this.gatewayPriorityLogicOutput.gatewayPriority = gatewayPriority
            this.gatewayPriorityLogicOutput.isEnforcement   = true
        }
    }

    def setGatewayReferenceIds(Map gatewayReferenceIds) {
        if(!this.gatewayPriorityLogicOutput){
            this.gatewayPriorityLogicOutput   = new GatewayPriorityLogicOutput([],false, [:])
        }
        if(gatewayReferenceIds!=null){
            this.gatewayPriorityLogicOutput.gatewayReferenceIds = gatewayReferenceIds
        }
    }

    def shuffle(List<?> input) {
        Collections.shuffle(input)
        return input
    }
}

class GatewayPriorityLogicOutput {
    List<String> gatewayPriority = []
    Boolean isEnforcement = false

    Map gatewayReferenceIds = [:]

    public GatewayPriorityLogicOutput(List<String> gatewayPriority, Boolean isEnforcement, Map gatewayReferenceIds){
        this.gatewayPriority = gatewayPriority
        this.isEnforcement   = isEnforcement
        this.gatewayReferenceIds = gatewayReferenceIds
    }

    // def asStringifyJson() {
    //     def message = [gateway_priority : this.gatewayPriority?.toString(),
    //                    is_enforcment    : this.isEnforcement

    //     ]
    //     return (message as JSON).toString().replaceAll("\n","")
    // }
}
// -------------------------------------------------------------------------

// https://bitbucket.org/juspay/graphh/src/287712eee2558500e507f45a7451e24f1bb898d2/grails-app/services/juspay/DslBuilderService.groovy?at=master#DslBuilderService.groovy-10
class DslBuilderService {
    private class CustomScriptSandbox extends GroovyValueFilter {
        @Override
        Object filter(Object o) {
            if (o instanceof Collection || o instanceof Map || o instanceof Number)
                return o;
            if (o == null || ALLOWED_TYPES.contains(o.class) || ALLOWED_TYPES.contains(o))
                return o;
            if (o instanceof Script || o instanceof Closure)
                return o; // access to properties of compiled groovy script
            throw new SecurityException("Oops, unexpected type: " + o.class);
        }

        private static final Set<Class> ALLOWED_TYPES = [
                Boolean,
                Integer,
                Character,
                String,
                Array,
                System,
                Random
        ] as Set
    }

    public ThreadSafeLRUMap scriptCache

    public GroovyShell shell; //threadsafe

    public DslBuilderService() {
        scriptCache = new ThreadSafeLRUMap(2000)
        def compilerConfig = new CompilerConfiguration()
        def secureAst = new SecureASTCustomizer()
        secureAst.with({
            methodDefinitionAllowed = false
        })

        compilerConfig.addCompilationCustomizers(new SandboxTransformer(), secureAst)
        shell = new GroovyShell(compilerConfig)
    }

    public def evaluateScript(Log log, String cacheDomain, String cacheID, String snippet) {
        def code = "{->${snippet}\n}"

        def sandbox = new CustomScriptSandbox()
        try {
            sandbox.register()
            Script script = getScript(cacheDomain, cacheID, code)
            synchronized (script) {
                return script.run()
            }
        } catch (RuntimeException e) {
            log.error("exception while running script", e)
            throw e
        } finally {
            sandbox.unregister()
        }
    }

    public Script getScript(String cacheDomain, String cacheID, String code) {
        String key = cacheDomain + "_" + cacheID + "_" + code.hashCode()
        Script script = scriptCache.get(key)
        if (!script) {
            script = shell.parse(code)
            if (script) {
                scriptCache.put(key, script)
            }
        }
        return script
    }
}

//----------------------------------------------------------------------------------------------------

// https://bitbucket.org/juspay/graphh/src/287712eee2558500e507f45a7451e24f1bb898d2/src/groovy/juspay/ThreadSafeLRUMap.groovy?at=master#ThreadSafeLRUMap.groovy-6
/**
 * Created by harish.r on 29/08/17.
 */
class ThreadSafeLRUMap {
    private Map cacheMap

    def ThreadSafeLRUMap(int mSize) {
        cacheMap = Collections.synchronizedMap(new LRUCache(100, mSize, 0.75F, true))
    }

    public def put(def key, def elem) {
        return cacheMap.put(key, elem)
    }

    public def get(def key) {
        return cacheMap.get(key)
    }
}

//https://bitbucket.org/juspay/graphh/src/287712eee2558500e507f45a7451e24f1bb898d2/src/groovy/juspay/LRUCache.groovy?at=master#lines-6
/**
 * Created by harish.r on 29/08/17.
 */
class LRUCache extends LinkedHashMap {
    private int cSize

    public LRUCache(int initialCapacity, int cSize, float loadFactor, boolean accessOrder) {
        super(initialCapacity, loadFactor, accessOrder)
        this.cSize = cSize
    }

    @Override
    protected boolean removeEldestEntry(Map.Entry eldest) {
        return size() >= this.cSize
    }
}

// -------------------------------------------------------------------------
// Log entry tags

class Log {
    def List<List<String>> log = []
    def error(String dscr, err) {
        log = log + [["Error", dscr, err.toString()]]
    }
    def info(String dscr) {
        log = log + [["Info", dscr]]
    }
}
// -------------------------------------------------------------------------
import org.codehaus.groovy.control.CompilerConfiguration
import org.codehaus.groovy.control.customizers.SecureASTCustomizer
import org.kohsuke.groovy.sandbox.SandboxTransformer
import org.kohsuke.groovy.sandbox.GroovyValueFilter
import org.kohsuke.groovy.sandbox.GroovyInterceptor
import org.codehaus.groovy.runtime.MethodClosure
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
                        dslBuilderService.evaluateScript(log, "gatewayPriorityLogic", merchantId, script, context)
                    } catch (ScriptEvaluationException e) {
                        def (label, userMessage) = ScriptEvaluationException.DESCRIPTIONS[e.errorCode]
                        http.sendResponseHeaders(400, 0)
                        log.error("${label} while trying to execute custom gateway priority logic for order ${orderInfo?.orderId}", e)
                        println("${label} while trying to execute custom gateway priority logic for order ${orderInfo?.orderId} : ${e}")
                        return out.write(JsonOutput.toJson(
                            [ error  : true
                            , error_message : e.errorCode
                            , user_message  : userMessage
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
                        dslBuilderService.evaluateScript(log, "gatewayPriorityLogic", merchantId, script, context)
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
            if (o instanceof MethodClosure)
                throw new SecurityException("Oops, unexpected type: " + o.class);
            if (o instanceof Collection || o instanceof Map || o instanceof Number || o instanceof GString)
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
                Random,
                Math,
                Date
        ] as Set

        // `System` itself is not an allowed type; only reading the clock is
        // permitted, for scripts that call it directly instead of using the
        // `currentTimeMillis` field on the context.
        @Override
        Object onStaticCall(GroovyInterceptor.Invoker invoker, Class receiver, String method, Object... args) throws Throwable {
            if (receiver == System && method == "currentTimeMillis" && args.length == 0) {
                return invoker.call(receiver, method)
            }
            checkMethod(method)
            return super.onStaticCall(invoker, receiver, method, args)
        }

        // The value filter checks receivers, arguments and return values, so a
        // method on an allowed type still runs before its result is checked.
        // These methods act outside the script (processes, code evaluation,
        // runtime metadata, dynamic dispatch) and are refused by name.
        @Override
        Object onMethodCall(GroovyInterceptor.Invoker invoker, Object receiver, String method, Object... args) throws Throwable {
            checkMethod(method)
            return super.onMethodCall(invoker, receiver, method, args)
        }

        private static final Set<String> DISALLOWED_METHODS = [
                "execute",
                "evaluate",
                "invokeMethod",
                "getMetaClass",
                "setMetaClass",
                "addShutdownHook",
                "forName",
                "sleep",
                "wait"
        ] as Set

        private static void checkMethod(String method) {
            if (DISALLOWED_METHODS.contains(method))
                throw new SecurityException("Oops, method not allowed: " + method);
        }
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

    // Compiles the snippet into a closure and invokes it with `context` as the
    // delegate. The closure must run while the value filter is registered: the
    // filter is thread-local and only applies to calls made while it is active.
    //
    // Anything the script hands back can contain code that runs when it is
    // read (a GString with a closure, a closure in a map, an exception whose
    // message renders script values). So, before the filter is unregistered,
    // the output is copied into plain values and failures are reduced to a
    // ScriptEvaluationException carrying only strings.
    public void evaluateScript(Log log, String cacheDomain, String cacheID, String snippet, Context context) {
        def code = "{->${snippet}\n}"

        def sandbox = new CustomScriptSandbox()
        try {
            sandbox.register()
            Script script = getScript(cacheDomain, cacheID, code)
            Closure closure
            synchronized (script) {
                closure = (Closure) script.run()
            }
            closure.delegate = context
            closure()
            context.gatewayPriorityLogicOutput = plainOutput(context.gatewayPriorityLogicOutput)
        } catch (Exception e) {
            def failure = new ScriptEvaluationException(errorCodeFor(e), describe(e))
            log.error("exception while running script", failure)
            throw failure
        } finally {
            sandbox.unregister()
        }
    }

    private static GatewayPriorityLogicOutput plainOutput(GatewayPriorityLogicOutput output) {
        if (output == null) {
            return new GatewayPriorityLogicOutput([], false, [:])
        }
        List gatewayPriority = output.gatewayPriority == null ? null : output.gatewayPriority.collect { plainValue(it) }
        Map gatewayReferenceIds = null
        if (output.gatewayReferenceIds != null) {
            gatewayReferenceIds = [:]
            output.gatewayReferenceIds.each { k, v -> gatewayReferenceIds.put(plainValue(k), plainValue(v)) }
        }
        return new GatewayPriorityLogicOutput(gatewayPriority, output.isEnforcement == true, gatewayReferenceIds)
    }

    private static Object plainValue(Object value) {
        if (value == null || value instanceof Number || value instanceof Boolean)
            return value
        if (value instanceof CharSequence || value instanceof Character)
            return value.toString()
        throw new SecurityException("Oops, unexpected type in script output: " + value.getClass())
    }

    private static String errorCodeFor(Exception e) {
        if (e instanceof IndexOutOfBoundsException) return ScriptEvaluationException.CODE_TOO_LARGE
        if (e instanceof GroovyRuntimeException) return ScriptEvaluationException.COMPILATION_ERROR
        return ScriptEvaluationException.UNHANDLED_EXCEPTION
    }

    private static String describe(Exception e) {
        try {
            return e.toString()
        } catch (Throwable ignored) {
            return e.getClass().getName()
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

// A script failure reduced to plain strings, so that handling it after the
// sandbox is unregistered cannot run script code. It keeps no cause or stack
// trace for the same reason.
class ScriptEvaluationException extends RuntimeException {
    static final String CODE_TOO_LARGE = "CODE_TOO_LARGE"
    static final String COMPILATION_ERROR = "COMPILATION_ERROR"
    static final String UNHANDLED_EXCEPTION = "UNHANDLED_EXCEPTION"

    // error code -> [log label, user message]
    static final Map<String, List<String>> DESCRIPTIONS = [
            (CODE_TOO_LARGE)     : ["IndexOutOfBoundsException", "Script cannot be executed due to huge size of method."],
            (COMPILATION_ERROR)  : ["GroovyRuntimeException", "Script cannot be executed due to compilation issues."],
            (UNHANDLED_EXCEPTION): ["Exception", "Script cannot be executed due to Exception"]
    ]

    final String errorCode

    ScriptEvaluationException(String errorCode, String message) {
        super(message, null, false, false)
        this.errorCode = errorCode
    }
}
// -------------------------------------------------------------------------
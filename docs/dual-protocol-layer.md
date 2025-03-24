# Dual Protocol Layer: gRPC and HTTP

## Overview

Dynamo implements a unique dual-protocol layer that allows services to be exposed via both gRPC and HTTP simultaneously. This enables clients to interact with Dynamo using their preferred protocol without requiring separate service implementations.

## Architecture

The implementation uses a custom generator that extends Tonic's standard gRPC code generation to automatically create corresponding HTTP endpoints for each gRPC service. This is done through a specialized WebGenerator in the build process.

### Key Components

- **Protocol Buff Definition**: Service interfaces are defined once in `.proto` files
- **Code Generation**: Build process generates both gRPC and HTTP handlers
- **Axum Integration**: HTTP endpoints are implemented using the Axum framework
- **Automatic Conversion**: Seamless serialization/deserialization between gRPC and HTTP formats

## How It Works

1. **Define Once**: Services are defined once using Protocol Buffers
2. **Generate Both**: Build scripts generate both gRPC service code and HTTP routes
3. **Single Implementation**: Service logic is implemented only once
4. **Configure at Runtime**: Choose protocol through configuration settings

The system automatically handles conversion between HTTP and gRPC formats, including:
- Converting gRPC metadata to HTTP headers and vice versa
- Mapping gRPC status codes to HTTP status codes
- Handling serialization differences between the protocols

## Benefits

1. **Protocol Flexibility**: Clients can choose between gRPC or HTTP based on their needs
2. **Unified Codebase**: Service logic is implemented only once
3. **No Duplication**: Avoid duplicating endpoint logic and type definitions
4. **Simple Configuration**: Switch between protocols with a single configuration option
5. **Testing Ease**: Test services using simple HTTP clients like cURL

## Using the Dual Protocol Layer

### Configuration

In your configuration file, specify the protocol:

```toml
[server]
type = "grpc" # or "http"
```

### Client Options

Clients can interact with the service using either protocol:

**gRPC Client**:
- Better performance for high-throughput scenarios
- Efficient binary serialization
- Built-in streaming support
- Strong typing via generated client code

**HTTP Client**:
- Works in environments where gRPC isn't supported (e.g., browsers)
- Simpler to debug and inspect with standard tools
- Easier integration with existing HTTP-based systems
- No need for protocol buffer compilation

## Technical Considerations

1. **Performance**: gRPC generally offers better performance for service-to-service communication
2. **Compatibility**: HTTP offers broader compatibility but without some gRPC advantages
3. **Streaming**: gRPC excels at bidirectional streaming; HTTP is more limited
4. **Headers/Metadata**: Slight differences in how metadata is handled between protocols

## Future Directions

- Enhanced streaming support for HTTP endpoints
- WebSocket alternatives for bidirectional communication in HTTP mode
- Automatic OpenAPI/Swagger documentation generation
- Support for protocol-specific optimizations

This innovative approach allows Dynamo to be more accessible to a wider range of clients while maintaining a clean and maintainable codebase.

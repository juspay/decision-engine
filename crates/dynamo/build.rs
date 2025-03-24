use std::{env, path::PathBuf};

use heck::ToSnakeCase;
use quote::quote;

mod cargo_workspace {
    /// Verify that the cargo metadata workspace packages format matches that expected by
    /// [`set_cargo_workspace_members_env`] to set the `CARGO_WORKSPACE_MEMBERS` environment variable.
    ///
    /// This function should be typically called within build scripts, before the
    /// [`set_cargo_workspace_members_env`] function is called.
    ///
    /// # Panics
    ///
    /// Panics if running the `cargo metadata` command fails, or if the workspace member package names
    /// cannot be determined.
    pub fn verify_cargo_metadata_format() {
        #[allow(clippy::expect_used)]
        let metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("Failed to obtain cargo metadata");

        assert!(
            metadata
                .workspace_packages()
                .iter()
                .any(|package| package.name == env!("CARGO_PKG_NAME")),
            "Unable to determine workspace member package names from `cargo metadata`"
        );
    }

    /// Sets the `CARGO_WORKSPACE_MEMBERS` environment variable to include a comma-separated list of
    /// names of all crates in the current cargo workspace.
    ///
    /// This function should be typically called within build scripts, so that the environment variable
    /// is available to the corresponding crate at compile time.
    ///
    /// # Panics
    ///
    /// Panics if running the `cargo metadata` command fails.
    #[allow(clippy::expect_used)]
    pub fn set_cargo_workspace_members_env() {
        use std::io::Write;

        let metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("Failed to obtain cargo metadata");

        let workspace_members = metadata
            .workspace_packages()
            .iter()
            .map(|package| package.name.as_str())
            .collect::<Vec<_>>()
            .join(",");

        writeln!(
            &mut std::io::stdout(),
            "cargo:rustc-env=CARGO_WORKSPACE_MEMBERS={workspace_members}"
        )
        .expect("Failed to set `CARGO_WORKSPACE_MEMBERS` environment variable");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    cargo_workspace::verify_cargo_metadata_format();
    cargo_workspace::set_cargo_workspace_members_env();

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let generator = tonic_build::configure().service_generator();

    let web_generator = WebGenerator {
        inner_generator: generator,
    };

    prost_build::Config::new()
        .service_generator(Box::new(web_generator))
        .file_descriptor_set_path(out_dir.join("dynamo_descriptor.bin"))
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(
            &[
                "proto/success_rate.proto",
                "proto/health_check.proto",
                "proto/elimination.proto",
                "proto/contract_routing.proto",
            ],
            &["proto"],
        )?;

    Ok(())
}

/// A service generator that creates web endpoints for gRPC services using Axum.
///
/// The `WebGenerator` wraps another service generator and extends its functionality
/// by generating additional code for web-based access to gRPC services. It creates
/// Axum routes that correspond to the gRPC service methods, allowing the service
/// to be accessed via HTTP POST requests.
///
/// # Implementation Details
///
/// The generator creates:
/// - An Axum router function for each gRPC service
/// - POST endpoints for each service method
/// - Automatic conversion between HTTP and gRPC requests/responses
/// - Proper handling of metadata and headers
/// - Error status conversion from gRPC to HTTP
///
/// # Example
///
/// Given this proto file:
/// ```protobuf
/// syntax = "proto3";
/// package user.v1;
///
/// service UserService {
///     rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
///     rpc GetUser(GetUserRequest) returns (GetUserResponse);
/// }
/// ```
///
/// The generator creates corresponding HTTP endpoints:
/// ```http
/// POST /user.v1.UserService/CreateUser
/// Content-Type: application/json
///
/// {
///   // CreateUserRequest fields in JSON format
/// }
///
/// POST /user.v1.UserService/GetUser
/// Content-Type: application/json
///
/// {
///   // GetUserRequest fields in JSON format
/// }
/// ```
///
/// # Type Parameters
///
/// The generated router function accepts any type that implements the service trait.
///
struct WebGenerator {
    /// The inner generator that handles the base gRPC code generation.
    /// This is typically the default Tonic generator.
    inner_generator: Box<dyn prost_build::ServiceGenerator>,
}

impl prost_build::ServiceGenerator for WebGenerator {
    fn generate(&mut self, service: prost_build::Service, buf: &mut String) {
        self.inner_generator.generate(service.clone(), buf);

        let package = &service.package;
        let name = &service.proto_name;
        let func_name = service.name.to_string();
        let ident_func_name = quote::format_ident!("{}", func_name);
        let branch_names = service
            .methods
            .iter()
            .map(|method| format!("/{package}.{name}/{}", method.proto_name))
            .collect::<Vec<_>>();

        let func_names = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.name))
            .collect::<Vec<_>>();

        let branch_request = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.input_type.trim_matches('"')))
            .collect::<Vec<_>>();

        let branch_response = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.output_type.trim_matches('"')))
            .collect::<Vec<_>>();

        let snake_case_name = func_name.to_snake_case();
        let service_name = quote::format_ident!("{}_handler", snake_case_name);
        let server_module = quote::format_ident!("{}_server", snake_case_name);

        let output = quote! {
            #[doc = "Axum Router for handling the gRPC service. This router is generated with the [`prost-build`] crate. This builds a web router on top of the gRPC service."]
            #[doc = ""]
            #[doc = ::std::concat!("Package: `", stringify!(#package), "`")]
            #[doc = ""]
            #[doc = ::std::concat!("Name: `", stringify!(#name), "`")]
            #[doc = ""]
            #[doc = "Routes:"]
            #(
                #[doc = ::std::concat!("- `", stringify!(#func_names), "` `::` [`", stringify!(#branch_request), "`]` -> `[`", stringify!(#branch_response), "`]")]
            )*
            pub fn #service_name<T: #server_module::#ident_func_name>(server: T) -> ::axum::Router {
                use ::axum::extract::State;
                use ::axum::response::IntoResponse;
                use std::sync::Arc;
                let router = ::axum::Router::new();

                #(
                    let router = router.route(#branch_names, ::axum::routing::post(|State(state): State<Arc<T>>, extension: ::http::Extensions, headers: ::http::header::HeaderMap, ::axum::Json(body): ::axum::Json<#branch_request>| async move {

                        let metadata_map = ::tonic::metadata::MetadataMap::from_headers(headers);
                        let request = ::tonic::Request::from_parts(metadata_map, extension, body);

                        let output = <T as #server_module::#ident_func_name>::#func_names(&state, request).await;

                        match output {
                            Ok(response) => {
                                let (metadata_map, body, extension) = response.into_parts();
                                let headers = metadata_map.into_headers();
                                let body = ::axum::Json(body);

                                (headers, extension, body).into_response()
                            },
                            Err(status) => {
                                let (parts, body) = status.into_http().into_parts();

                                ::http::response::Response::from_parts(parts, ::axum::body::Body::new(body))
                            }
                        }

                    }));
                )*

                router.with_state(Arc::new(server))
            }
        };

        buf.push_str(&output.to_string());
    }
    fn finalize(&mut self, _buf: &mut String) {
        self.inner_generator.finalize(_buf);
    }

    fn finalize_package(&mut self, _package: &str, _buf: &mut String) {
        self.inner_generator.finalize_package(_package, _buf);
    }
}

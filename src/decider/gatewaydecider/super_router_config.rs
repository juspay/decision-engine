use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Struct representing the super router constants configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SuperRouterConstants {
    pub success_rate_delta: f64,
}

impl SuperRouterConstants {
    /// Default value for success_rate_delta if JSON file cannot be read
    const DEFAULT_SUCCESS_RATE_DELTA: f64 = 0.05;
    
    /// Path to the super router constants JSON file
    const CONFIG_FILE_PATH: &'static str = "super_router_constants.json";
    
    /// Reads the super router constants from the JSON file
    /// Returns the default value if the file cannot be read or parsed
    pub fn read_from_file() -> SuperRouterConstants {
        Self::read_from_file_with_path(Self::CONFIG_FILE_PATH)
    }
    
    /// Reads the super router constants from a JSON file at the specified path
    /// Returns the default value if the file cannot be read or parsed
    pub fn read_from_file_with_path<P: AsRef<Path>>(path: P) -> SuperRouterConstants {
        match File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(config) => {
                        tracing::info!("Successfully loaded super router constants from JSON file");
                        config
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse super router constants JSON file: {}. Using default values.",
                            e
                        );
                        SuperRouterConstants {
                            success_rate_delta: Self::DEFAULT_SUCCESS_RATE_DELTA,
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to open super router constants JSON file: {}. Using default values.",
                    e
                );
                SuperRouterConstants {
                    success_rate_delta: Self::DEFAULT_SUCCESS_RATE_DELTA,
                }
            }
        }
    }
}

/// Function to read the super router constants and return the success_rate_delta value
/// Returns an f64 value that can be used in the super router logic
pub fn read_super_router_constants() -> f64 {
    let config = SuperRouterConstants::read_from_file();
    config.success_rate_delta
}

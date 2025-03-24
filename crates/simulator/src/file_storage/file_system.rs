//! Module for local file system storage operations

use std::{
    fs::{remove_file, File},
    io::{Read, Write},
    path::PathBuf,
};

use error_stack::{Result, ResultExt};

use crate::file_storage::{FileStorageError, FileStorageInterface};

const SIMULATION_DATA_DIRECTORY: &str = "simulation_data";

/// Constructs the file path for a given file key within the file system.
/// The file path is generated based on the workspace path and the provided file key.
fn get_file_path(file_key: impl AsRef<str>) -> PathBuf {
    let mut file_path = PathBuf::new();
    file_path.push(std::env::current_dir().unwrap_or(".".into()));

    file_path.push(SIMULATION_DATA_DIRECTORY);
    file_path.push(file_key.as_ref());
    file_path
}

/// Represents a file system for storing and managing files locally.
#[derive(Debug, Clone)]
pub(super) struct FileSystem;

impl FileSystem {
    /// Saves the provided file data to the file system under the specified file key.
    async fn upload_file(
        &self,
        file_key: &str,
        file: Vec<u8>,
    ) -> Result<(), FileSystemStorageError> {
        let file_path = get_file_path(file_key);

        // Ignore the file name and create directories in the `file_path` if not exists
        std::fs::create_dir_all(
            file_path
                .parent()
                .ok_or(FileSystemStorageError::CreateDirFailed)
                .attach_printable("Failed to obtain parent directory")?,
        )
        .change_context(FileSystemStorageError::CreateDirFailed)?;

        let mut file_handler =
            File::create(file_path).change_context(FileSystemStorageError::CreateFailure)?;
        file_handler
            .write_all(&file)
            .change_context(FileSystemStorageError::WriteFailure)
    }

    /// Deletes the file associated with the specified file key from the file system.
    async fn delete_file(&self, file_key: &str) -> Result<(), FileSystemStorageError> {
        let file_path = get_file_path(file_key);
        remove_file(file_path).change_context(FileSystemStorageError::DeleteFailure)
    }

    /// Retrieves the file content associated with the specified file key from the file system.
    async fn retrieve_file(&self, file_key: &str) -> Result<Vec<u8>, FileSystemStorageError> {
        let mut received_data: Vec<u8> = Vec::new();
        let file_path = get_file_path(file_key);
        let mut file =
            File::open(file_path).change_context(FileSystemStorageError::FileOpenFailure)?;
        file.read_to_end(&mut received_data)
            .change_context(FileSystemStorageError::ReadFailure)?;
        Ok(received_data)
    }

    /// Deletes the directory associated with the specified directory key from the file system.
    async fn delete_directory(&self, dir_key: &str) -> Result<Vec<String>, FileSystemStorageError> {
        let dir_path = get_file_path(dir_key);

        let files_to_be_deleted = std::fs::read_dir(&dir_path).ok();

        let mut deleted_keys = vec![];
        if let Some(files_to_be_deleted) = files_to_be_deleted {
            files_to_be_deleted.flatten().for_each(|entry| {
                let file = entry.path().display().to_string();
                deleted_keys.push(transform_path(&file));
            });
        }

        std::fs::remove_dir_all(dir_path).ok();

        Ok(deleted_keys)
    }
}

#[async_trait::async_trait]
impl FileStorageInterface for FileSystem {
    /// Saves the provided file data to the file system under the specified file key.
    async fn upload_file(&self, file_key: &str, file: Vec<u8>) -> Result<(), FileStorageError> {
        self.upload_file(file_key, file)
            .await
            .change_context(FileStorageError::UploadFailed)
    }

    /// Deletes the file associated with the specified file key from the file system.
    async fn delete_file(&self, file_key: &str) -> Result<(), FileStorageError> {
        self.delete_file(file_key)
            .await
            .change_context(FileStorageError::DeleteFailed)
    }

    /// Retrieves the file content associated with the specified file key from the file system.
    async fn retrieve_file(&self, file_key: &str) -> Result<Vec<u8>, FileStorageError> {
        self.retrieve_file(file_key)
            .await
            .change_context(FileStorageError::RetrieveFailed)
    }

    /// Deletes the directory associated with the specified directory key from the file system.
    async fn delete_directory(&self, dir_key: &str) -> Result<Vec<String>, FileStorageError> {
        self.delete_directory(dir_key)
            .await
            .change_context(FileStorageError::DeleteDirectoryFailed)
    }
}

/// Represents an error that can occur during local file system storage operations.
#[derive(Debug, thiserror::Error)]
enum FileSystemStorageError {
    /// Error indicating opening a file failed
    #[error("Failed while opening the file")]
    FileOpenFailure,

    /// Error indicating file creation failed.
    #[error("Failed to create file")]
    CreateFailure,

    /// Error indicating reading a file failed.
    #[error("Failed while reading the file")]
    ReadFailure,

    /// Error indicating writing to a file failed.
    #[error("Failed while writing into file")]
    WriteFailure,

    /// Error indicating file deletion failed.
    #[error("Failed while deleting the file")]
    DeleteFailure,

    /// Error indicating directory creation failed
    #[error("Failed while creating a directory")]
    CreateDirFailed,
}

fn transform_path(path: &str) -> String {
    // Remove everything before and including "SIMULATION_DATA_DIRECTORY"
    let after_root_dir = match path.split(&format!("{SIMULATION_DATA_DIRECTORY}/")).nth(1) {
        Some(s) => s,
        None => path, // In case "SIMULATION_DATA_DIRECTORY/" is not found
    };

    // Split by "/" and remove the second element "DETAILED_REPORT_DIRECTORY_NAME"
    let parts: Vec<&str> = after_root_dir.split('/').collect();

    // Combine first and third+ parts
    let merchant_id = parts[1];
    let filename_parts: Vec<&str> = parts[3..].to_vec();
    let filename = filename_parts.join("/");

    // Remove .json at the end
    let without_extension = filename.trim_end_matches(".json");

    // Combine with merchant name and replace / with :
    format!("{}:{}", merchant_id, without_extension)
}

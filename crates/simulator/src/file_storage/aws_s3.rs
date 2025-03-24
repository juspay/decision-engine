use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{
    operation::{
        delete_object::DeleteObjectError, delete_objects::DeleteObjectsError,
        get_object::GetObjectError, list_objects_v2::ListObjectsV2Error,
        put_object::PutObjectError,
    },
    types::{Delete, ObjectIdentifier},
    Client,
};
use aws_sdk_sts::config::Region;
use error_stack::{Result, ResultExt};

use super::InvalidFileStorageConfig;
use crate::file_storage::{FileStorageError, FileStorageInterface};

/// Configuration for AWS S3 file storage.
#[derive(Debug, serde::Deserialize, Clone, Default)]
#[serde(default)]
pub struct AwsFileStorageConfig {
    /// The AWS region to send file uploads
    region: String,
    /// The AWS s3 bucket to send file uploads
    bucket_name: String,
}

impl AwsFileStorageConfig {
    /// Validates the AWS S3 file storage configuration.
    pub(super) fn validate(&self) -> Result<(), InvalidFileStorageConfig> {
        if self.region.is_empty() || self.region.trim().is_empty() {
            return Err(InvalidFileStorageConfig("aws s3 region must not be empty").into());
        }

        if self.bucket_name.is_empty() || self.bucket_name.trim().is_empty() {
            return Err(InvalidFileStorageConfig("aws s3 bucket name must not be empty").into());
        }

        Ok(())
    }
}

/// AWS S3 file storage client.
#[derive(Debug, Clone)]
pub(super) struct AwsFileStorageClient {
    /// AWS S3 client
    inner_client: Client,
    /// The name of the AWS S3 bucket.
    bucket_name: String,
}

impl AwsFileStorageClient {
    /// Creates a new AWS S3 file storage client.
    pub(super) async fn new(config: &AwsFileStorageConfig) -> Self {
        let region_provider = RegionProviderChain::first_try(Region::new(config.region.clone()));
        let sdk_config = aws_config::from_env().region(region_provider).load().await;
        Self {
            inner_client: Client::new(&sdk_config),
            bucket_name: config.bucket_name.clone(),
        }
    }

    /// Uploads a file to AWS S3.
    async fn upload_file(&self, file_key: &str, file: Vec<u8>) -> Result<(), AwsS3StorageError> {
        self.inner_client
            .put_object()
            .bucket(&self.bucket_name)
            .key(file_key)
            .body(file.into())
            .send()
            .await
            .map_err(AwsS3StorageError::UploadFailure)?;
        Ok(())
    }

    /// Deletes a file from AWS S3.
    async fn delete_file(&self, file_key: &str) -> Result<(), AwsS3StorageError> {
        self.inner_client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(file_key)
            .send()
            .await
            .map_err(AwsS3StorageError::DeleteFailure)?;
        Ok(())
    }

    /// Retrieves a file from AWS S3.
    async fn retrieve_file(&self, file_key: &str) -> Result<Vec<u8>, AwsS3StorageError> {
        Ok(self
            .inner_client
            .get_object()
            .bucket(&self.bucket_name)
            .key(file_key)
            .send()
            .await
            .map_err(AwsS3StorageError::RetrieveFailure)?
            .body
            .collect()
            .await
            .map_err(AwsS3StorageError::UnknownError)?
            .to_vec())
    }

    /// Deletes a directory contents from AWS S3.
    async fn delete_directory_contents(
        &self,
        dir_key: &str,
    ) -> Result<Vec<String>, AwsS3StorageError> {
        let objects = self
            .inner_client
            .list_objects_v2()
            .bucket(&self.bucket_name)
            .prefix(dir_key)
            .send()
            .await
            .map_err(AwsS3StorageError::ListFilesFailure)?;

        let keys = objects.contents.map(|contents| {
            contents
                .into_iter()
                .filter_map(|obj| obj.key)
                .collect::<Vec<String>>()
        });

        let keys = match keys {
            Some(keys) if keys.is_empty() => return Ok(Vec::new()),
            Some(keys) => keys,
            None => return Ok(Vec::new()),
        };

        let mut objects_to_delete = Vec::with_capacity(keys.len());
        let mut keys_to_delete = Vec::with_capacity(keys.len());

        for key in keys {
            let object_id = ObjectIdentifier::builder()
                .key(&key)
                .build()
                .map_err(AwsS3StorageError::FailedToBuildObjects)?;

            objects_to_delete.push(object_id);
            keys_to_delete.push(key);
        }

        if !objects_to_delete.is_empty() {
            self.inner_client
                .delete_objects()
                .bucket(&self.bucket_name)
                .delete(
                    Delete::builder()
                        .set_objects(Some(objects_to_delete))
                        .build()
                        .map_err(AwsS3StorageError::FailedToBuildObjects)?,
                )
                .send()
                .await
                .map_err(AwsS3StorageError::DeleteDirectoryFailure)?;
        }
        keys_to_delete = keys_to_delete
            .into_iter()
            .filter_map(|key| transform_path(&key))
            .collect::<Vec<_>>();
        Ok(keys_to_delete)
    }
}

#[async_trait::async_trait]
impl FileStorageInterface for AwsFileStorageClient {
    /// Uploads a file to AWS S3.
    async fn upload_file(&self, file_key: &str, file: Vec<u8>) -> Result<(), FileStorageError> {
        self.upload_file(file_key, file)
            .await
            .change_context(FileStorageError::UploadFailed)?;
        Ok(())
    }

    /// Deletes a file from AWS S3.
    async fn delete_file(&self, file_key: &str) -> Result<(), FileStorageError> {
        self.delete_file(file_key)
            .await
            .change_context(FileStorageError::DeleteFailed)?;
        Ok(())
    }

    /// Retrieves a file from AWS S3.
    async fn retrieve_file(&self, file_key: &str) -> Result<Vec<u8>, FileStorageError> {
        Ok(self
            .retrieve_file(file_key)
            .await
            .change_context(FileStorageError::RetrieveFailed)?)
    }

    /// Deletes a directory contents from AWS S3.
    async fn delete_directory(&self, dir_key: &str) -> Result<Vec<String>, FileStorageError> {
        self.delete_directory_contents(dir_key)
            .await
            .change_context(FileStorageError::DeleteDirectoryFailed)
    }
}

/// Enum representing errors that can occur during AWS S3 file storage operations.
#[derive(Debug, thiserror::Error)]
enum AwsS3StorageError {
    /// Error indicating that file upload to S3 failed.
    #[error("File upload to S3 failed: {0:?}")]
    UploadFailure(aws_sdk_s3::error::SdkError<PutObjectError>),

    /// Error indicating that file retrieval from S3 failed.
    #[error("File retrieve from S3 failed: {0:?}")]
    RetrieveFailure(aws_sdk_s3::error::SdkError<GetObjectError>),

    /// Error indicating that listing files from S3 failed.
    #[error("List files from S3 failed: {0:?}")]
    ListFilesFailure(aws_sdk_s3::error::SdkError<ListObjectsV2Error>),

    /// Error indicating that file deletion from S3 failed.
    #[error("File delete from S3 failed: {0:?}")]
    DeleteFailure(aws_sdk_s3::error::SdkError<DeleteObjectError>),

    /// Error indicating that directory deletion from S3 failed.
    #[error("Directory delete from S3 failed: {0:?}")]
    DeleteDirectoryFailure(aws_sdk_s3::error::SdkError<DeleteObjectsError>),

    /// Failed to build object identifier.
    #[error("Failed to build objects: {0:?}")]
    FailedToBuildObjects(aws_sdk_s3::error::BuildError),

    /// Unknown error occurred.
    #[error("Unknown error occurred: {0:?}")]
    UnknownError(aws_sdk_s3::primitives::ByteStreamError),
}

fn transform_path(input: &str) -> Option<String> {
    let parts: Vec<&str> = input.split('/').collect();
    let merchant = parts[1];
    let report = parts[3].strip_suffix(".json")?;
    Some(format!("{}:{}", merchant, report))
}

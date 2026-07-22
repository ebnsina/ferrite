//! S3-compatible storage (MinIO / S3 / R2) behind one API. Endpoint + path-style
//! toggle handles MinIO; callers don't see the difference.

use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("presigning failed: {0}")]
    Presign(String),
    #[error("upload failed: {0}")]
    Upload(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Configuration for the object-storage backend.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub bucket: String,
    /// Custom endpoint (e.g. `http://localhost:9000` for MinIO). `None` uses
    /// the AWS default resolver.
    pub endpoint_url: Option<String>,
    /// MinIO needs path-style addressing (`endpoint/bucket/key`); real S3 does not.
    pub force_path_style: bool,
}

/// A thin, cloneable handle over an S3-compatible bucket.
#[derive(Clone)]
pub struct Storage {
    client: Client,
    bucket: String,
}

impl Storage {
    /// Build a client from the ambient AWS environment plus our overrides.
    /// Credentials/region come from the standard AWS config chain.
    pub async fn connect(cfg: StorageConfig) -> Result<Self, StorageError> {
        if cfg.bucket.is_empty() {
            return Err(StorageError::Config("bucket name is empty".into()));
        }

        let shared = aws_config::load_from_env().await;
        let mut builder =
            aws_sdk_s3::config::Builder::from(&shared).force_path_style(cfg.force_path_style);

        if let Some(endpoint) = cfg.endpoint_url.as_deref() {
            builder = builder.endpoint_url(endpoint);
        }

        Ok(Self {
            client: Client::from_conf(builder.build()),
            bucket: cfg.bucket,
        })
    }

    /// Presigned URL a browser can `PUT` an upload to directly (single-part).
    /// Multipart presigning for large files is added in Phase 2.
    pub async fn presign_put(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, StorageError> {
        let cfg = PresigningConfig::expires_in(expires_in)
            .map_err(|e| StorageError::Presign(e.to_string()))?;

        let req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(cfg)
            .await
            .map_err(|e| StorageError::Presign(e.to_string()))?;

        Ok(req.uri().to_string())
    }

    /// Presigned URL to `GET` an object (e.g. a playlist for playback/QC).
    pub async fn presign_get(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, StorageError> {
        let cfg = PresigningConfig::expires_in(expires_in)
            .map_err(|e| StorageError::Presign(e.to_string()))?;

        let req = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(cfg)
            .await
            .map_err(|e| StorageError::Presign(e.to_string()))?;

        Ok(req.uri().to_string())
    }

    /// Upload a local file (used by workers to push rendition artifacts).
    pub async fn put_file(&self, key: &str, path: &str) -> Result<(), StorageError> {
        let body = ByteStream::from_path(path)
            .await
            .map_err(|e| StorageError::Upload(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| StorageError::Upload(e.to_string()))?;

        Ok(())
    }

    /// Download an object to a local path (used by workers to fetch the source).
    pub async fn get_file(&self, key: &str, dest_path: &str) -> Result<(), StorageError> {
        let mut object = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::Download(e.to_string()))?;

        let mut file = tokio::fs::File::create(dest_path)
            .await
            .map_err(|e| StorageError::Download(e.to_string()))?;

        // Stream chunks to disk rather than buffering the whole object in memory.
        while let Some(chunk) = object
            .body
            .try_next()
            .await
            .map_err(|e| StorageError::Download(e.to_string()))?
        {
            file.write_all(&chunk)
                .await
                .map_err(|e| StorageError::Download(e.to_string()))?;
        }

        file.flush()
            .await
            .map_err(|e| StorageError::Download(e.to_string()))?;

        Ok(())
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }
}

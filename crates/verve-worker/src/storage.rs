//! Object storage via OpenDAL, so the provider is a config change.
//!
//! Layout is `{tenant}/assets/{asset_id}/{rendition}/seg-*` — an edge log line
//! attributes to tenant, asset and rung by path alone, with no lookup.

use opendal::{Operator, services};
use std::collections::HashMap;

/// What went wrong talking to storage.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The configuration named a provider or bucket we cannot use.
    #[error("storage config: {0}")]
    Config(String),
    /// The operation itself failed.
    #[error(transparent)]
    Backend(#[from] opendal::Error),
}

/// Where bytes live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// S3-compatible. MinIO locally, anything else in production.
    S3 {
        /// Bucket name.
        bucket: String,
        /// Region. MinIO ignores it but the SDK requires one.
        region: String,
        /// Endpoint URL. Empty means AWS's default.
        endpoint: String,
        /// Access key id.
        access_key: String,
        /// Secret access key.
        secret_key: String,
    },
    /// A local directory. Tests and single-machine development.
    Fs {
        /// Directory root.
        root: String,
    },
}

impl Backend {
    /// Read a backend from environment variables.
    ///
    /// `VERVE_STORAGE=fs` uses `VERVE_STORAGE_ROOT`; anything else is S3.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, StorageError> {
        if var("VERVE_STORAGE").as_deref() == Some("fs") {
            return Ok(Self::Fs {
                root: var("VERVE_STORAGE_ROOT").unwrap_or_else(|| "/var/lib/verve/objects".into()),
            });
        }
        Ok(Self::S3 {
            bucket: var("VERVE_S3_BUCKET")
                .ok_or_else(|| StorageError::Config("VERVE_S3_BUCKET is unset".into()))?,
            region: var("VERVE_S3_REGION").unwrap_or_else(|| "us-east-1".into()),
            endpoint: var("VERVE_S3_ENDPOINT").unwrap_or_default(),
            access_key: var("VERVE_S3_ACCESS_KEY").unwrap_or_default(),
            secret_key: var("VERVE_S3_SECRET_KEY").unwrap_or_default(),
        })
    }

    /// Read a backend from the process environment.
    pub fn from_env() -> Result<Self, StorageError> {
        Self::from_vars(|k| std::env::var(k).ok())
    }
}

/// Build an operator for `backend`.
pub fn operator(backend: &Backend) -> Result<Operator, StorageError> {
    let op = match backend {
        Backend::S3 {
            bucket,
            region,
            endpoint,
            access_key,
            secret_key,
        } => {
            let mut builder = services::S3::default()
                .bucket(bucket)
                .region(region)
                .access_key_id(access_key)
                .secret_access_key(secret_key);
            if !endpoint.is_empty() {
                builder = builder.endpoint(endpoint);
            }
            Operator::new(builder)?
        }
        Backend::Fs { root } => Operator::new(services::Fs::default().root(root))?,
    };
    Ok(op)
}

/// Where a rendition's segments live for `tenant` and `asset`.
pub fn rendition_prefix(tenant: &str, asset: &str, rendition: &str) -> String {
    format!("{tenant}/assets/{asset}/{rendition}/")
}

/// Where a source's original bytes live.
pub fn source_key(tenant: &str, source: &str) -> String {
    format!("{tenant}/sources/{source}")
}

/// Backends keyed by `storage_region`, since a tenant is bound to one forever.
#[derive(Debug, Default)]
pub struct RegionMap(HashMap<String, Backend>);

impl RegionMap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `region` to `backend`.
    pub fn insert(&mut self, region: impl Into<String>, backend: Backend) -> &mut Self {
        self.0.insert(region.into(), backend);
        self
    }

    /// The backend for `region`, if one is configured.
    pub fn get(&self, region: &str) -> Option<&Backend> {
        self.0.get(region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_tenant_asset_and_rung_without_a_lookup() {
        assert_eq!(
            rendition_prefix("t_abc", "as_123", "1080p"),
            "t_abc/assets/as_123/1080p/"
        );
        assert_eq!(source_key("t_abc", "src_9"), "t_abc/sources/src_9");
    }

    #[test]
    fn s3_without_a_bucket_fails_at_config_not_at_first_write() {
        let err = Backend::from_vars(|_| None).unwrap_err();
        assert!(matches!(err, StorageError::Config(_)), "{err}");
    }

    #[test]
    fn fs_needs_nothing_but_a_root() {
        let b = Backend::from_vars(|k| match k {
            "VERVE_STORAGE" => Some("fs".into()),
            "VERVE_STORAGE_ROOT" => Some("/tmp/verve".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            b,
            Backend::Fs {
                root: "/tmp/verve".into()
            }
        );
        operator(&b).unwrap();
    }

    #[test]
    fn a_tenant_reaches_only_its_own_regions_backend() {
        let mut map = RegionMap::new();
        map.insert("eu-central", Backend::Fs { root: "/eu".into() });
        assert!(map.get("eu-central").is_some());
        assert!(map.get("us-east").is_none());
    }
}

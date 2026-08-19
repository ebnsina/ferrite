//! Object storage via OpenDAL, so the provider is a config change.
//!
//! Layout is `{tenant}/assets/{asset_id}/{rendition}/seg-*` — an edge log line
//! attributes to tenant, asset and rung by path alone, with no lookup.

use opendal::{Operator, services};

/// What went wrong talking to storage.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// A required variable is unset or empty.
    #[error("{0} is unset")]
    Missing(&'static str),
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
    /// Every value is required. A defaulted endpoint or an empty credential
    /// fails at the first write against the wrong bucket, not at startup.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, StorageError> {
        let required = |name: &'static str| {
            var(name)
                .filter(|v| !v.trim().is_empty())
                .ok_or(StorageError::Missing(name))
        };

        if var("FERRITE_STORAGE").as_deref() == Some("fs") {
            return Ok(Self::Fs {
                root: required("FERRITE_STORAGE_ROOT")?,
            });
        }
        Ok(Self::S3 {
            bucket: required("FERRITE_S3_BUCKET")?,
            region: required("FERRITE_S3_REGION")?,
            endpoint: required("FERRITE_S3_ENDPOINT")?,
            access_key: required("FERRITE_S3_ACCESS_KEY")?,
            secret_key: required("FERRITE_S3_SECRET_KEY")?,
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

    fn s3_env(skip: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |k| {
            if k == skip {
                return None;
            }
            match k {
                "FERRITE_S3_BUCKET" => Some("ferrite-assets".to_string()),
                "FERRITE_S3_REGION" => Some("us-east-1".to_string()),
                "FERRITE_S3_ENDPOINT" => Some("http://localhost:9020".to_string()),
                "FERRITE_S3_ACCESS_KEY" => Some("key".to_string()),
                "FERRITE_S3_SECRET_KEY" => Some("secret".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn every_s3_variable_is_required() {
        // A defaulted region or an empty credential fails at the first write
        // against the wrong bucket, which is a far worse place to find out.
        for name in [
            "FERRITE_S3_BUCKET",
            "FERRITE_S3_REGION",
            "FERRITE_S3_ENDPOINT",
            "FERRITE_S3_ACCESS_KEY",
            "FERRITE_S3_SECRET_KEY",
        ] {
            let err = Backend::from_vars(s3_env(name)).unwrap_err();
            assert!(
                matches!(err, StorageError::Missing(m) if m == name),
                "{name}: {err}"
            );
        }
        assert!(Backend::from_vars(s3_env("")).is_ok());
    }

    #[test]
    fn a_blank_variable_counts_as_unset() {
        let blank = |k: &str| {
            Some(if k == "FERRITE_S3_BUCKET" {
                "  ".into()
            } else {
                "x".into()
            })
        };
        assert!(matches!(
            Backend::from_vars(blank).unwrap_err(),
            StorageError::Missing("FERRITE_S3_BUCKET")
        ));
    }

    #[test]
    fn fs_needs_an_explicit_root() {
        let no_root = |k: &str| (k == "FERRITE_STORAGE").then(|| "fs".to_string());
        assert!(Backend::from_vars(no_root).is_err());
    }

    #[test]
    fn fs_takes_the_root_it_is_given() {
        let b = Backend::from_vars(|k| match k {
            "FERRITE_STORAGE" => Some("fs".into()),
            "FERRITE_STORAGE_ROOT" => Some("/tmp/ferrite".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            b,
            Backend::Fs {
                root: "/tmp/ferrite".into()
            }
        );
        operator(&b).unwrap();
    }
}

//! Tracing and OpenTelemetry wiring, shared by every binary.
//! One trace from upload to every chunk means one place that configures it.

#![warn(missing_docs)]

use anyhow::{Context as _, Result};
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Configuration that could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A required variable is unset.
    #[error("{0} is unset")]
    Missing(&'static str),
}

/// How logs are printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Human-readable. The default for a terminal.
    #[default]
    Pretty,
    /// One JSON object per line, for the fleet.
    Json,
}

impl LogFormat {
    /// Parse `FERRITE_LOG_FORMAT`. Anything unrecognised is pretty.
    pub fn parse(value: &str) -> Self {
        match value {
            "json" => Self::Json,
            _ => Self::Pretty,
        }
    }
}

/// What to wire up.
#[derive(Debug, Clone)]
pub struct Config {
    /// `service.name` on every span. Use the crate's binary name.
    pub service_name: String,
    /// `service.version`.
    pub service_version: String,
    /// Deployment environment: `dev`, `staging`, `prod`.
    pub environment: String,
    /// OTLP collector endpoint. `None` disables export and keeps logs.
    pub otlp_endpoint: Option<String>,
    /// Log format.
    pub log_format: LogFormat,
    /// `RUST_LOG`-style filter.
    pub filter: String,
}

impl Config {
    /// A config for `service_name`, with everything else from the environment.
    pub fn from_env(service_name: impl Into<String>) -> Result<Self, ConfigError> {
        Self::from_vars(service_name, |k| std::env::var(k).ok())
    }

    /// `from_env` with the environment injected, so it can be tested.
    ///
    /// Nothing is defaulted. A service that silently logs at `info` in
    /// production because `RUST_LOG` was misspelt is worse than one that stops.
    pub fn from_vars(
        service_name: impl Into<String>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            service_name: service_name.into(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: var("FERRITE_ENV").ok_or(ConfigError::Missing("FERRITE_ENV"))?,
            // Unset means no collector, not localhost: a CLI on a laptop must not
            // spend startup retrying a connection nobody asked for.
            otlp_endpoint: var("OTEL_EXPORTER_OTLP_ENDPOINT"),
            log_format: LogFormat::parse(
                var("FERRITE_LOG_FORMAT")
                    .ok_or(ConfigError::Missing("FERRITE_LOG_FORMAT"))?
                    .as_str(),
            ),
            filter: var("RUST_LOG").ok_or(ConfigError::Missing("RUST_LOG"))?,
        })
    }
}

/// Shuts telemetry down when dropped. Hold it for the life of the process.
#[derive(Debug)]
#[must_use = "dropping this immediately flushes and shuts telemetry down"]
pub struct Guard {
    provider: Option<SdkTracerProvider>,
}

impl Guard {
    /// Flush pending spans and shut down. Called on drop; explicit is clearer.
    pub fn shutdown(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(e) = provider.shutdown()
        {
            eprintln!("ferrite-telemetry: shutdown failed: {e}");
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Install the global subscriber. Call once, first thing in `main`.
pub fn init(config: Config) -> Result<Guard> {
    let filter = EnvFilter::try_new(&config.filter)
        .with_context(|| format!("invalid log filter {:?}", config.filter))?;

    let fmt = tracing_subscriber::fmt::layer();
    let fmt = match config.log_format {
        LogFormat::Json => fmt.json().flatten_event(true).boxed(),
        LogFormat::Pretty => fmt.compact().boxed(),
    };

    let provider = match &config.otlp_endpoint {
        Some(endpoint) => Some(tracer_provider(&config, endpoint)?),
        None => None,
    };

    let otel = provider
        .as_ref()
        .map(|p| tracing_opentelemetry::layer().with_tracer(p.tracer(config.service_name.clone())));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt)
        .with(otel)
        .try_init()
        .context("a tracing subscriber is already installed")?;

    Ok(Guard { provider })
}

fn tracer_provider(config: &Config, endpoint: &str) -> Result<SdkTracerProvider> {
    use opentelemetry_otlp::WithExportConfig as _;
    use opentelemetry_semantic_conventions::resource;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .with_context(|| format!("cannot reach OTLP collector at {endpoint}"))?;

    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new(resource::SERVICE_NAME, config.service_name.clone()),
            KeyValue::new(resource::SERVICE_VERSION, config.service_version.clone()),
            KeyValue::new(
                resource::DEPLOYMENT_ENVIRONMENT_NAME,
                config.environment.clone(),
            ),
        ])
        .build();

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(skip: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |k| {
            if k == skip {
                return None;
            }
            match k {
                "FERRITE_ENV" => Some("prod".to_string()),
                "RUST_LOG" => Some("info".to_string()),
                "FERRITE_LOG_FORMAT" => Some("json".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn every_variable_is_required() {
        // A service quietly logging at info in production because RUST_LOG was
        // misspelt is worse than one that refuses to start.
        for name in ["FERRITE_ENV", "RUST_LOG", "FERRITE_LOG_FORMAT"] {
            let err = Config::from_vars("ferrite-cli", env(name)).unwrap_err();
            assert_eq!(err, ConfigError::Missing(name));
        }
    }

    #[test]
    fn no_endpoint_means_no_exporter_not_a_default_localhost() {
        let c = Config::from_vars("ferrite-cli", env("")).unwrap();
        assert!(c.otlp_endpoint.is_none());
        assert_eq!(c.environment, "prod");
        assert_eq!(c.log_format, LogFormat::Json);
    }

    #[test]
    fn an_unrecognised_format_falls_back_rather_than_failing() {
        // The only default left: a wrong format string still prints logs.
        assert_eq!(LogFormat::parse("yaml"), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("json"), LogFormat::Json);
    }

    #[test]
    fn a_bad_filter_is_rejected_rather_than_silently_ignored() {
        let mut c = Config::from_vars("ferrite-test", env("")).unwrap();
        c.filter = "not a filter=====".into();
        assert!(init(c).is_err());
    }
}

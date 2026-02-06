//! Prometheus metrics exporter
//!
//! Exposes metrics via HTTP for Prometheus scraping.

use crate::metrics::recorder::init_metrics;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;
use std::sync::OnceLock;

/// Global prometheus handle
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Metrics server configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Address to bind the metrics server
    pub listen_addr: SocketAddr,

    /// Path for metrics endpoint (default: "/metrics")
    pub endpoint: String,

    /// Whether to include process metrics
    pub include_process_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9090".parse().unwrap(),
            endpoint: "/metrics".to_string(),
            include_process_metrics: true,
        }
    }
}

impl MetricsConfig {
    /// Create a new config with custom address
    pub fn with_addr(addr: SocketAddr) -> Self {
        Self {
            listen_addr: addr,
            ..Default::default()
        }
    }
}

/// Initialize and start the metrics exporter
///
/// Returns the handle to render metrics manually if needed.
/// This function can only be called once; subsequent calls return the existing handle.
pub fn start_metrics_server(
    config: MetricsConfig,
) -> Result<&'static PrometheusHandle, MetricsError> {
    // Initialize metric descriptions
    init_metrics();

    // Try to set up the prometheus exporter
    if let Some(handle) = PROMETHEUS_HANDLE.get() {
        return Ok(handle);
    }

    let builder = PrometheusBuilder::new();

    // Install the recorder
    let handle = builder
        .with_http_listener(config.listen_addr)
        .install_recorder()
        .map_err(|e| MetricsError::SetupFailed(e.to_string()))?;

    // Store the handle
    let _ = PROMETHEUS_HANDLE.set(handle);

    Ok(PROMETHEUS_HANDLE.get().unwrap())
}

/// Get the current prometheus handle (if initialized)
pub fn get_handle() -> Option<&'static PrometheusHandle> {
    PROMETHEUS_HANDLE.get()
}

/// Render metrics as a string (for custom endpoints)
pub fn render_metrics() -> Option<String> {
    PROMETHEUS_HANDLE.get().map(|h| h.render())
}

/// Errors that can occur during metrics setup
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Failed to setup metrics: {0}")]
    SetupFailed(String),

    #[error("Metrics already initialized")]
    AlreadyInitialized,
}

/// Create an axum route for serving metrics
///
/// Use this if you want to integrate metrics into an existing axum server.
pub fn metrics_route() -> axum::routing::MethodRouter {
    use axum::response::IntoResponse;

    axum::routing::get(|| async {
        match render_metrics() {
            Some(metrics) => (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                )],
                metrics,
            )
                .into_response(),
            None => (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Metrics not initialized",
            )
                .into_response(),
        }
    })
}

/// Builder for metrics configuration
pub struct MetricsBuilder {
    config: MetricsConfig,
}

impl MetricsBuilder {
    pub fn new() -> Self {
        Self {
            config: MetricsConfig::default(),
        }
    }

    /// Set the listen address
    pub fn listen_addr(mut self, addr: SocketAddr) -> Self {
        self.config.listen_addr = addr;
        self
    }

    /// Set the metrics endpoint path
    pub fn endpoint(mut self, path: impl Into<String>) -> Self {
        self.config.endpoint = path.into();
        self
    }

    /// Enable/disable process metrics
    pub fn process_metrics(mut self, enabled: bool) -> Self {
        self.config.include_process_metrics = enabled;
        self
    }

    /// Build and start the metrics server
    pub fn build(self) -> Result<&'static PrometheusHandle, MetricsError> {
        start_metrics_server(self.config)
    }
}

impl Default for MetricsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert_eq!(config.endpoint, "/metrics");
        assert!(config.include_process_metrics);
    }

    #[test]
    fn test_metrics_builder() {
        let builder = MetricsBuilder::new()
            .listen_addr("127.0.0.1:9191".parse().unwrap())
            .endpoint("/custom-metrics")
            .process_metrics(false);

        assert_eq!(builder.config.listen_addr.port(), 9191);
        assert_eq!(builder.config.endpoint, "/custom-metrics");
        assert!(!builder.config.include_process_metrics);
    }

    // Note: Can't easily test start_metrics_server in unit tests due to global state
    // Integration tests should verify the actual HTTP endpoint
}

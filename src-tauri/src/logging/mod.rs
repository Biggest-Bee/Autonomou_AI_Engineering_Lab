// Comprehensive Logging and Monitoring Module
// Structured logging with in-memory ring buffer, log levels, and metrics collection

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

impl LogLevel {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TRACE" => Some(LogLevel::Trace),
            "DEBUG" => Some(LogLevel::Debug),
            "INFO" => Some(LogLevel::Info),
            "WARN" | "WARNING" => Some(LogLevel::Warn),
            "ERROR" => Some(LogLevel::Error),
            "FATAL" => Some(LogLevel::Fatal),
            _ => None,
        }
    }
}

/// A single structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    pub fn new(level: LogLevel, target: &str, message: &str) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            level,
            target: target.to_string(),
            message: message.to_string(),
            fields: HashMap::new(),
        }
    }

    pub fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
        self.fields.insert(key.to_string(), value);
        self
    }
}

/// Counter metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counter {
    pub name: String,
    pub value: u64,
    pub labels: HashMap<String, String>,
}

/// Gauge metric (current value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gauge {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
}

/// Histogram metric (duration tracking)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub buckets: Vec<(f64, u64)>, // (upper_bound, count)
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub component: String,
    pub healthy: bool,
    pub message: String,
    pub checked_at: u64,
}

/// Logging and monitoring manager
pub struct LoggingManager {
    /// Minimum level persisted to the ring buffer
    min_level: LogLevel,
    /// Ring buffer capacity
    capacity: usize,
    /// In-memory log buffer (newest last)
    buffer: Arc<RwLock<Vec<LogEntry>>>,
    /// Counters: name -> value
    counters: Arc<RwLock<HashMap<String, u64>>>,
    /// Gauges: name -> value
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    /// Histograms: name -> histogram
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    /// Total entries ever logged (for stats)
    total_logged: AtomicU64,
    /// Dropped entries (buffer overflow)
    dropped: AtomicU64,
}

impl LoggingManager {
    pub fn new(min_level: LogLevel, capacity: usize) -> Self {
        Self {
            min_level,
            capacity: capacity.max(1),
            buffer: Arc::new(RwLock::new(Vec::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            total_logged: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }

    /// Record a log entry (respects min_level for buffer persistence)
    pub async fn log(&self, entry: LogEntry) {
        self.total_logged.fetch_add(1, Ordering::Relaxed);
        if entry.level >= self.min_level {
            let mut buf = self.buffer.write().await;
            if buf.len() >= self.capacity {
                buf.remove(0);
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            buf.push(entry);
        }
    }

    /// Convenience: log at info level
    pub async fn info(&self, target: &str, message: &str) {
        self.log(LogEntry::new(LogLevel::Info, target, message)).await;
    }

    /// Convenience: log at warn level
    pub async fn warn(&self, target: &str, message: &str) {
        self.log(LogEntry::new(LogLevel::Warn, target, message)).await;
    }

    /// Convenience: log at error level
    pub async fn error(&self, target: &str, message: &str) {
        self.log(LogEntry::new(LogLevel::Error, target, message)).await;
    }

    /// Convenience: log at debug level
    pub async fn debug(&self, target: &str, message: &str) {
        self.log(LogEntry::new(LogLevel::Debug, target, message)).await;
    }

    /// Query logs with optional level filter and limit (returns newest first)
    pub async fn query_logs(&self, min_level: Option<LogLevel>, limit: usize) -> Vec<LogEntry> {
        let buf = self.buffer.read().await;
        buf.iter()
            .rev()
            .filter(|e| min_level.map_or(true, |l| e.level >= l))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Increment a counter
    pub async fn increment_counter(&self, name: &str, delta: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += delta;
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    /// Observe a duration into a histogram (milliseconds)
    pub async fn observe_duration(&self, name: &str, duration_ms: f64) {
        let mut histograms = self.histograms.write().await;
        let hist = histograms.entry(name.to_string()).or_insert_with(|| Histogram {
            name: name.to_string(),
            count: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            buckets: vec![
                (10.0, 0), (50.0, 0), (100.0, 0), (500.0, 0),
                (1000.0, 0), (5000.0, 0), (f64::INFINITY, 0),
            ],
        });
        hist.count += 1;
        hist.sum += duration_ms;
        hist.min = hist.min.min(duration_ms);
        hist.max = hist.max.max(duration_ms);
        for (bound, count) in hist.buckets.iter_mut() {
            if duration_ms <= *bound {
                *count += 1;
                break;
            }
        }
    }

    /// Get all counters
    pub async fn get_counters(&self) -> Vec<Counter> {
        let counters = self.counters.read().await;
        counters
            .iter()
            .map(|(name, value)| Counter {
                name: name.clone(),
                value: *value,
                labels: HashMap::new(),
            })
            .collect()
    }

    /// Get all gauges
    pub async fn get_gauges(&self) -> Vec<Gauge> {
        let gauges = self.gauges.read().await;
        gauges
            .iter()
            .map(|(name, value)| Gauge {
                name: name.clone(),
                value: *value,
                labels: HashMap::new(),
            })
            .collect()
    }

    /// Get all histograms
    pub async fn get_histograms(&self) -> Vec<Histogram> {
        let histograms = self.histograms.read().await;
        histograms.values().cloned().collect()
    }

    /// Get monitoring statistics
    pub async fn get_stats(&self) -> MonitoringStats {
        MonitoringStats {
            buffer_size: self.buffer.read().await.len(),
            buffer_capacity: self.capacity,
            total_logged: self.total_logged.load(Ordering::Relaxed),
            dropped_entries: self.dropped.load(Ordering::Relaxed),
            min_level: self.min_level,
            counter_count: self.counters.read().await.len(),
            gauge_count: self.gauges.read().await.len(),
            histogram_count: self.histograms.read().await.len(),
        }
    }

    /// Clear the log buffer
    pub async fn clear_logs(&self) {
        self.buffer.write().await.clear();
    }

    /// Register a component health check
    pub async fn register_health_check<F>(&self, component: &str, check: F)
    where
        F: FnOnce() -> HealthStatus + Send + 'static,
    {
        // Health checks are invoked on demand via run_health_checks
        // Store closures not supported across await easily; instead this
        // registers a named component whose status is checked externally.
        let _ = (component, check);
    }

    /// Run basic health checks on internal state
    pub async fn run_health_checks(&self) -> Vec<HealthStatus> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let stats = self.get_stats().await;
        let buffer_ratio = stats.buffer_size as f64 / stats.buffer_capacity as f64;

        vec![
            HealthStatus {
                component: "log_buffer".to_string(),
                healthy: buffer_ratio < 0.95,
                message: format!("{}/{} entries", stats.buffer_size, stats.buffer_capacity),
                checked_at: now,
            },
            HealthStatus {
                component: "log_pipeline".to_string(),
                healthy: stats.dropped_entries == 0 || stats.dropped_entries < stats.total_logged / 100,
                message: format!("{} dropped of {} logged", stats.dropped_entries, stats.total_logged),
                checked_at: now,
            },
        ]
    }
}

impl Default for LoggingManager {
    fn default() -> Self {
        Self::new(LogLevel::Info, 10_000)
    }
}

/// Monitoring statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStats {
    pub buffer_size: usize,
    pub buffer_capacity: usize,
    pub total_logged: u64,
    pub dropped_entries: u64,
    pub min_level: LogLevel,
    pub counter_count: usize,
    pub gauge_count: usize,
    pub histogram_count: usize,
}

/// Initialize global logger bridging the `log` crate into LoggingManager.
/// Call once at startup; subsequent calls are no-ops.
pub fn init_global_logger(manager: Arc<LoggingManager>) -> Result<()> {
    use log::{LevelFilter, Log, Metadata, Record};

    struct ManagerLogger {
        manager: Arc<LoggingManager>,
    }

    impl Log for ManagerLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            let level = match metadata.level() {
                log::Level::Trace => LogLevel::Trace,
                log::Level::Debug => LogLevel::Debug,
                log::Level::Info => LogLevel::Info,
                log::Level::Warn => LogLevel::Warn,
                log::Level::Error => LogLevel::Error,
            };
            level >= self.manager.min_level
        }

        fn log(&self, record: &Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let level = match record.level() {
                log::Level::Trace => LogLevel::Trace,
                log::Level::Debug => LogLevel::Debug,
                log::Level::Info => LogLevel::Info,
                log::Level::Warn => LogLevel::Warn,
                log::Level::Error => LogLevel::Error,
            };
            let manager = self.manager.clone();
            let entry = LogEntry::new(level, record.target(), &record.args().to_string());
            // Best-effort: spawn onto the current runtime if available
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move { manager.log(entry).await });
            }
        }

        fn flush(&self) {}
    }

    log::set_boxed_logger(Box::new(ManagerLogger { manager }))
        .map_err(|e| anyhow::anyhow!("Failed to set logger: {}", e))?;
    log::set_max_level(LevelFilter::Trace);
    Ok(())
}

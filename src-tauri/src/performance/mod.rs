// Performance Optimization Module
// TTL cache, operation timing, and performance benchmarking

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// A cache entry with TTL
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: u64, // unix millis
    hits: u64,
}

/// Generic TTL cache
pub struct TtlCache<V: Clone + Send + Sync + 'static> {
    entries: Arc<RwLock<HashMap<String, CacheEntry<V>>>>,
    default_ttl_ms: u64,
    max_entries: usize,
    hits: Arc<std::sync::atomic::AtomicU64>,
    misses: Arc<std::sync::atomic::AtomicU64>,
}

impl<V: Clone + Send + Sync + 'static> TtlCache<V> {
    pub fn new(default_ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            default_ttl_ms: default_ttl.as_millis() as u64,
            max_entries: max_entries.max(1),
            hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Insert a value with optional custom TTL
    pub async fn insert(&self, key: &str, value: V, ttl: Option<Duration>) {
        let ttl_ms = ttl.map(|d| d.as_millis() as u64).unwrap_or(self.default_ttl_ms);
        let mut entries = self.entries.write().await;

        // Evict if at capacity: drop expired first, then oldest
        if entries.len() >= self.max_entries && !entries.contains_key(key) {
            let now = Self::now_ms();
            entries.retain(|_, e| e.expires_at > now);
            if entries.len() >= self.max_entries {
                // Remove arbitrary entry (simple FIFO-ish eviction)
                if let Some(k) = entries.keys().next().cloned() {
                    entries.remove(&k);
                }
            }
        }

        entries.insert(
            key.to_string(),
            CacheEntry {
                value,
                expires_at: Self::now_ms() + ttl_ms,
                hits: 0,
            },
        );
    }

    /// Get a value if present and not expired
    pub async fn get(&self, key: &str) -> Option<V> {
        let now = Self::now_ms();
        let mut entries = self.entries.write().await;
        match entries.get_mut(key) {
            Some(entry) if entry.expires_at > now => {
                entry.hits += 1;
                self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Some(entry.value.clone())
            }
            Some(_) => {
                entries.remove(key);
                self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                None
            }
            None => {
                self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                None
            }
        }
    }

    /// Get or compute: returns cached value or computes, caches, and returns it
    pub async fn get_or_insert_with<F, Fut>(&self, key: &str, f: F) -> Result<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        if let Some(v) = self.get(key).await {
            return Ok(v);
        }
        let value = f().await?;
        self.insert(key, value.clone(), None).await;
        Ok(value)
    }

    pub async fn remove(&self, key: &str) -> Option<V> {
        self.entries.write().await.remove(key).map(|e| e.value)
    }

    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    /// Remove all expired entries; returns count removed
    pub async fn evict_expired(&self) -> usize {
        let now = Self::now_ms();
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|_, e| e.expires_at > now);
        before - entries.len()
    }

    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.misses.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// A recorded operation measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetric {
    pub name: String,
    pub duration_ms: f64,
    pub timestamp: u64,
    pub success: bool,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Performance monitor tracking operation timings
pub struct PerformanceMonitor {
    /// Recent metrics (bounded)
    metrics: Arc<RwLock<Vec<OperationMetric>>>,
    max_metrics: usize,
    /// Per-operation aggregates
    aggregates: Arc<RwLock<HashMap<String, OperationAggregate>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationAggregate {
    pub name: String,
    pub count: u64,
    pub total_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub errors: u64,
}

impl PerformanceMonitor {
    pub fn new(max_metrics: usize) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
            max_metrics: max_metrics.max(1),
            aggregates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Time an async operation and record the result
    pub async fn time<F, Fut, T>(&self, name: &str, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed().as_secs_f64() * 1000.0;
        let success = result.is_ok();
        self.record(OperationMetric {
            name: name.to_string(),
            duration_ms: duration,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            success,
            metadata: HashMap::new(),
        })
        .await;
        result
    }

    /// Record a pre-measured operation
    pub async fn record(&self, metric: OperationMetric) {
        {
            let mut metrics = self.metrics.write().await;
            if metrics.len() >= self.max_metrics {
                metrics.remove(0);
            }
            metrics.push(metric.clone());
        }

        let mut aggs = self.aggregates.write().await;
        let agg = aggs.entry(metric.name.clone()).or_insert_with(|| OperationAggregate {
            name: metric.name.clone(),
            count: 0,
            total_ms: 0.0,
            min_ms: f64::INFINITY,
            max_ms: f64::NEG_INFINITY,
            errors: 0,
        });
        agg.count += 1;
        agg.total_ms += metric.duration_ms;
        agg.min_ms = agg.min_ms.min(metric.duration_ms);
        agg.max_ms = agg.max_ms.max(metric.duration_ms);
        if !metric.success {
            agg.errors += 1;
        }
    }

    /// Get aggregate stats for all operations
    pub async fn get_aggregates(&self) -> Vec<OperationAggregate> {
        let aggs = self.aggregates.read().await;
        aggs.values()
            .map(|a| OperationAggregate {
                name: a.name.clone(),
                count: a.count,
                total_ms: a.total_ms,
                min_ms: if a.count == 0 { 0.0 } else { a.min_ms },
                max_ms: if a.count == 0 { 0.0 } else { a.max_ms },
                errors: a.errors,
            })
            .collect()
    }

    /// Get aggregate for a specific operation
    pub async fn get_aggregate(&self, name: &str) -> Option<OperationAggregate> {
        let aggs = self.aggregates.read().await;
        aggs.get(name).map(|a| OperationAggregate {
            name: a.name.clone(),
            count: a.count,
            total_ms: a.total_ms,
            min_ms: a.min_ms,
            max_ms: a.max_ms,
            errors: a.errors,
        })
    }

    /// Average duration in ms for an operation
    pub async fn avg_duration_ms(&self, name: &str) -> Option<f64> {
        self.get_aggregate(name)
            .await
            .filter(|a| a.count > 0)
            .map(|a| a.total_ms / a.count as f64)
    }

    /// Get recent raw metrics
    pub async fn get_recent_metrics(&self, limit: usize) -> Vec<OperationMetric> {
        let metrics = self.metrics.read().await;
        metrics.iter().rev().take(limit).cloned().collect()
    }

    /// Get slowest operations by average duration
    pub async fn get_slowest_operations(&self, limit: usize) -> Vec<OperationAggregate> {
        let mut aggs = self.get_aggregates().await;
        aggs.retain(|a| a.count > 0);
        aggs.sort_by(|a, b| {
            (b.total_ms / b.count as f64)
                .partial_cmp(&(a.total_ms / a.count as f64))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        aggs.truncate(limit);
        aggs
    }

    pub async fn clear(&self) {
        self.metrics.write().await.clear();
        self.aggregates.write().await.clear();
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// Benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u64,
    pub total_ms: f64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub throughput_ops_per_sec: f64,
}

/// Run a benchmark: executes `f` `iterations` times and reports stats
pub async fn run_benchmark<F, Fut, T>(
    name: &str,
    iterations: u64,
    mut f: F,
) -> Result<BenchmarkResult>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let iterations = iterations.max(1);
    let start = Instant::now();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;

    for _ in 0..iterations {
        let op_start = Instant::now();
        f().await?;
        let d = op_start.elapsed().as_secs_f64() * 1000.0;
        min = min.min(d);
        max = max.max(d);
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchmarkResult {
        name: name.to_string(),
        iterations,
        total_ms,
        avg_ms: total_ms / iterations as f64,
        min_ms: if min.is_infinite() { 0.0 } else { min },
        max_ms: if max.is_infinite() { 0.0 } else { max },
        throughput_ops_per_sec: if total_ms > 0.0 {
            iterations as f64 / (total_ms / 1000.0)
        } else {
            0.0
        },
    })
}

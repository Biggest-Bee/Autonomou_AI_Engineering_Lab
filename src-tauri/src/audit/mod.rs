// Advanced Security Auditing Module
// Append-only, tamper-evident audit trail for security-relevant events

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Categories of auditable events
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditEventType {
    // Authentication
    LoginSuccess,
    LoginFailure,
    Logout,
    TokenRefresh,
    // Authorization
    PermissionGranted,
    PermissionDenied,
    RoleChanged,
    // Code execution
    CodeExecuted,
    SandboxViolation,
    PrivilegedCommand,
    // Data access
    FileRead,
    FileWrite,
    FileDelete,
    SensitiveDataAccess,
    // System
    ConfigChanged,
    UserCreated,
    UserDeleted,
    ServiceStarted,
    ServiceStopped,
    // Security
    ViolationRecorded,
    RateLimitExceeded,
    SuspiciousActivity,
    InjectionAttempt,
    // Custom
    Custom(String),
}

/// Severity of an audit event
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    Info = 0,
    Notice = 1,
    Warning = 2,
    Critical = 3,
}

/// A single audit record with hash chaining for tamper evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub timestamp: u64,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
    pub actor: String,           // user_id or "system"
    pub action: String,          // human-readable action description
    pub resource: Option<String>,// affected resource identifier
    pub details: HashMap<String, serde_json::Value>,
    pub source_ip: Option<String>,
    pub prev_hash: String,       // hash of previous record (genesis = "0"*64)
    pub record_hash: String,     // sha256 of canonical fields + prev_hash
}

impl AuditRecord {
    /// Compute the record hash over canonical content
    fn compute_hash(
        sequence: u64,
        timestamp: u64,
        event_type: &AuditEventType,
        severity: AuditSeverity,
        actor: &str,
        action: &str,
        resource: &Option<String>,
        details: &HashMap<String, serde_json::Value>,
        source_ip: &Option<String>,
        prev_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sequence.to_le_bytes());
        hasher.update(timestamp.to_le_bytes());
        hasher.update(serde_json::to_string(event_type).unwrap_or_default());
        hasher.update((severity as u8).to_le_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(action.as_bytes());
        if let Some(r) = resource {
            hasher.update(r.as_bytes());
        }
        // Sort detail keys for canonical ordering
        let mut keys: Vec<_> = details.keys().collect();
        keys.sort();
        for k in keys {
            hasher.update(k.as_bytes());
            hasher.update(serde_json::to_string(&details[k]).unwrap_or_default());
        }
        if let Some(ip) = source_ip {
            hasher.update(ip.as_bytes());
        }
        hasher.update(prev_hash.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Filter for querying audit records
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub event_types: Option<Vec<AuditEventType>>,
    pub severities: Option<Vec<AuditSeverity>>,
    pub actor: Option<String>,
    pub after_timestamp: Option<u64>,
    pub before_timestamp: Option<u64>,
    pub resource: Option<String>,
    pub limit: Option<usize>,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_records: u64,
    pub by_event_type: HashMap<String, u64>,
    pub by_severity: HashMap<String, u64>,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
    pub chain_valid: bool,
}

/// Tamper-evident audit logger
pub struct AuditLogger {
    records: Arc<RwLock<Vec<AuditRecord>>>,
    sequence: Arc<std::sync::atomic::AtomicU64>,
    /// Maximum records retained in memory
    max_records: usize,
}

impl AuditLogger {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            max_records: max_records.max(1),
        }
    }

    /// Record an audit event
    pub async fn record(
        &self,
        event_type: AuditEventType,
        severity: AuditSeverity,
        actor: &str,
        action: &str,
        resource: Option<String>,
        details: HashMap<String, serde_json::Value>,
        source_ip: Option<String>,
    ) -> Result<AuditRecord> {
        let seq = self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let prev_hash = {
            let records = self.records.read().await;
            records.last().map(|r| r.record_hash.clone()).unwrap_or_else(|| "0".repeat(64))
        };

        let record_hash = AuditRecord::compute_hash(
            seq, timestamp, &event_type, severity, actor, action,
            &resource, &details, &source_ip, &prev_hash,
        );

        let record = AuditRecord {
            sequence: seq,
            timestamp,
            event_type,
            severity,
            actor: actor.to_string(),
            action: action.to_string(),
            resource,
            details,
            source_ip,
            prev_hash,
            record_hash,
        };

        {
            let mut records = self.records.write().await;
            if records.len() >= self.max_records {
                records.remove(0);
            }
            records.push(record.clone());
        }

        Ok(record)
    }

    /// Convenience: record with minimal fields
    pub async fn log_event(
        &self,
        event_type: AuditEventType,
        actor: &str,
        action: &str,
    ) -> Result<AuditRecord> {
        let severity = match event_type {
            AuditEventType::LoginFailure
            | AuditEventType::PermissionDenied
            | AuditEventType::SandboxViolation
            | AuditEventType::InjectionAttempt => AuditSeverity::Warning,
            AuditEventType::ViolationRecorded
            | AuditEventType::RateLimitExceeded
            | AuditEventType::SuspiciousActivity
            | AuditEventType::PrivilegedCommand => AuditSeverity::Critical,
            _ => AuditSeverity::Info,
        };
        self.record(event_type, severity, actor, action, None, HashMap::new(), None)
            .await
    }

    /// Query records matching a filter (returns newest first)
    pub async fn query(&self, filter: &AuditFilter) -> Vec<AuditRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .rev()
            .filter(|r| {
                if let Some(types) = &filter.event_types {
                    if !types.contains(&r.event_type) {
                        return false;
                    }
                }
                if let Some(sevs) = &filter.severities {
                    if !sevs.contains(&r.severity) {
                        return false;
                    }
                }
                if let Some(actor) = &filter.actor {
                    if &r.actor != actor {
                        return false;
                    }
                }
                if let Some(after) = filter.after_timestamp {
                    if r.timestamp < after {
                        return false;
                    }
                }
                if let Some(before) = filter.before_timestamp {
                    if r.timestamp > before {
                        return false;
                    }
                }
                if let Some(resource) = &filter.resource {
                    if r.resource.as_ref() != Some(resource) {
                        return false;
                    }
                }
                true
            })
            .take(filter.limit.unwrap_or(usize::MAX))
            .cloned()
            .collect()
    }

    /// Get a single record by sequence number
    pub async fn get_by_sequence(&self, seq: u64) -> Option<AuditRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.sequence == seq).cloned()
    }

    /// Verify the integrity of the hash chain.
    /// NOTE: only verifies records currently in the buffer (oldest may be truncated).
    pub async fn verify_chain(&self) -> bool {
        let records = self.records.read().await;
        if records.len() < 2 {
            return true;
        }
        for window in records.windows(2) {
            let (prev, curr) = (&window[0], &window[1]);
            if curr.prev_hash != prev.record_hash {
                return false;
            }
            // Recompute hash
            let recomputed = AuditRecord::compute_hash(
                curr.sequence, curr.timestamp, &curr.event_type, curr.severity,
                &curr.actor, &curr.action, &curr.resource, &curr.details,
                &curr.source_ip, &curr.prev_hash,
            );
            if recomputed != curr.record_hash {
                return false;
            }
        }
        // Verify last record too
        let last = records.last().unwrap();
        let recomputed = AuditRecord::compute_hash(
            last.sequence, last.timestamp, &last.event_type, last.severity,
            &last.actor, &last.action, &last.resource, &last.details,
            &last.source_ip, &last.prev_hash,
        );
        recomputed == last.record_hash
    }

    /// Get audit statistics
    pub async fn get_stats(&self) -> AuditStats {
        let records = self.records.read().await;
        let mut by_event_type: HashMap<String, u64> = HashMap::new();
        let mut by_severity: HashMap<String, u64> = HashMap::new();
        let mut oldest = None;
        let mut newest = None;

        for r in records.iter() {
            *by_event_type
                .entry(format!("{:?}", r.event_type))
                .or_insert(0) += 1;
            *by_severity
                .entry(format!("{:?}", r.severity))
                .or_insert(0) += 1;
            oldest = Some(oldest.map_or(r.timestamp, |o: u64| o.min(r.timestamp)));
            newest = Some(newest.map_or(r.timestamp, |n: u64| n.max(r.timestamp)));
        }

        AuditStats {
            total_records: records.len() as u64,
            by_event_type,
            by_severity,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
            chain_valid: true, // populated by caller via verify_chain
        }
    }

    /// Export all records (for persistence)
    pub async fn export_all(&self) -> Vec<AuditRecord> {
        self.records.read().await.clone()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(100_000)
    }
}

// Violation Tracker Module
// Implements 3-strike rule with Firestore backend for security violations

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Violation record stored in Firestore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationRecord {
    pub user_id: String,
    pub violation_count: u32,
    pub last_violation_time: String, // ISO 8601 timestamp
    pub ban_until: Option<String>,   // ISO 8601 timestamp for temporary bans
    pub is_permanently_banned: bool,
    pub secret_identifier: String, // Hidden from user
}

impl ViolationRecord {
    pub fn new(user_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            user_id,
            violation_count: 0,
            last_violation_time: now.to_string(),
            ban_until: None,
            is_permanently_banned: false,
            secret_identifier: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Violation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViolationType {
    SystemPromptLeakage,
    SecurityStringLeakage,
    JailbreakAttempt,
    UnauthorizedCommand,
}

impl ViolationType {
    pub fn from_error_message(message: &str) -> Self {
        if message.contains("security string detected") {
            ViolationType::SecurityStringLeakage
        } else if message.contains("system prompt") {
            ViolationType::SystemPromptLeakage
        } else if message.contains("jailbreak") || message.contains("ignore instructions") {
            ViolationType::JailbreakAttempt
        } else {
            ViolationType::UnauthorizedCommand
        }
    }
}

/// Violation check result
#[derive(Debug, Clone)]
pub struct ViolationCheck {
    pub is_allowed: bool,
    pub error_message: Option<String>,
    pub ban_duration: Option<Duration>,
}

impl ViolationCheck {
    pub fn allowed() -> Self {
        Self {
            is_allowed: true,
            error_message: None,
            ban_duration: None,
        }
    }

    pub fn blocked(message: String, duration: Option<Duration>) -> Self {
        Self {
            is_allowed: false,
            error_message: Some(message),
            ban_duration: duration,
        }
    }
}

/// Violation tracker with Firestore backend
pub struct ViolationTracker {
    // Local cache for violations (synced with Firestore)
    local_violations: Arc<RwLock<HashMap<String, ViolationRecord>>>,
    firestore_client: Option<Arc<FirestoreClient>>,
}

/// Firestore client for admin-only access (placeholder)
pub struct FirestoreClient {
    // In production, this would contain Firebase Firestore client
    pub _placeholder: (),
}

impl FirestoreClient {
    pub async fn get_violation_record(&self, _user_id: &str) -> Result<Option<ViolationRecord>> {
        // Mock implementation - returns None for new users
        Ok(None)
    }

    pub async fn update_violation_record(&self, _record: &ViolationRecord) -> Result<()> {
        // Mock implementation - succeeds without actual Firestore call
        Ok(())
    }

    pub async fn create_violation_record(&self, _record: &ViolationRecord) -> Result<()> {
        // Mock implementation - succeeds without actual Firestore call
        Ok(())
    }
}

impl ViolationTracker {
    pub fn new() -> Self {
        Self {
            local_violations: Arc::new(RwLock::new(HashMap::new())),
            firestore_client: None, // Will be initialized with real client in production
        }
    }

    pub fn with_firestore_client(client: Arc<FirestoreClient>) -> Self {
        Self {
            local_violations: Arc::new(RwLock::new(HashMap::new())),
            firestore_client: Some(client),
        }
    }

    /// Check if user is allowed to make requests (not banned)
    pub async fn check_user_allowed(&self, user_id: &str) -> Result<ViolationCheck> {
        // Try to get from local cache first
        let local_violations = self.local_violations.read().await;
        if let Some(record) = local_violations.get(user_id) {
            if record.is_permanently_banned {
                return Ok(ViolationCheck::blocked(
                    "Error".to_string(),
                    None,
                ));
            }

            if let Some(ban_until_str) = &record.ban_until {
                if let Ok(ban_until_seconds) = ban_until_str.parse::<u64>() {
                    let now_seconds = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    
                    if now_seconds < ban_until_seconds {
                        let duration = Duration::from_secs(ban_until_seconds - now_seconds);
                        return Ok(ViolationCheck::blocked(
                            "Error".to_string(),
                            Some(duration),
                        ));
                    }
                }
            }
        }

        // If not in local cache, try Firestore
        if let Some(client) = &self.firestore_client {
            if let Some(record) = client.get_violation_record(user_id).await? {
                // Update local cache
                let mut local_violations = self.local_violations.write().await;
                local_violations.insert(user_id.to_string(), record.clone());

                // Check ban status
                if record.is_permanently_banned {
                    return Ok(ViolationCheck::blocked(
                        "Error".to_string(),
                        None,
                    ));
                }

                if let Some(ban_until_str) = &record.ban_until {
                    if let Ok(ban_until_seconds) = ban_until_str.parse::<u64>() {
                        let now_seconds = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        
                        if now_seconds < ban_until_seconds {
                            let duration = Duration::from_secs(ban_until_seconds - now_seconds);
                            return Ok(ViolationCheck::blocked(
                                "Error".to_string(),
                                Some(duration),
                            ));
                        }
                    }
                }
            }
        }

        Ok(ViolationCheck::allowed())
    }

    /// Record a security violation
    pub async fn record_violation(&self, user_id: &str, _violation_type: ViolationType) -> Result<ViolationCheck> {
        let mut local_violations = self.local_violations.write().await;
        
        // Check if user exists before mutable borrow
        let user_exists = local_violations.contains_key(user_id);
        
        // Get or create violation record
        let record = local_violations.entry(user_id.to_string())
            .or_insert_with(|| ViolationRecord::new(user_id.to_string()));

        record.violation_count += 1;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        record.last_violation_time = now.to_string();

        // Apply 3-strike rule
        let check_result = match record.violation_count {
            1 => {
                // First violation - just record it
                ViolationCheck::allowed()
            }
            2 => {
                // Second violation - 24 hour timeout
                let ban_until = now + (24 * 3600);
                record.ban_until = Some(ban_until.to_string());
                ViolationCheck::blocked(
                    "Error".to_string(),
                    Some(Duration::from_secs(24 * 3600)),
                )
            }
            3 | _ => {
                // Third violation - permanent ban
                record.is_permanently_banned = true;
                record.ban_until = None;
                ViolationCheck::blocked(
                    "Error".to_string(),
                    None,
                )
            }
        };

        // Sync with Firestore
        if let Some(client) = &self.firestore_client {
            if user_exists {
                client.update_violation_record(record).await?;
            } else {
                client.create_violation_record(record).await?;
            }
        }

        Ok(check_result)
    }

    /// Get violation count for a user (admin only)
    pub async fn get_violation_count(&self, user_id: &str) -> Result<u32> {
        let local_violations = self.local_violations.read().await;
        
        if let Some(record) = local_violations.get(user_id) {
            Ok(record.violation_count)
        } else {
            // Try Firestore
            if let Some(client) = &self.firestore_client {
                if let Some(record) = client.get_violation_record(user_id).await? {
                    let mut local_violations = self.local_violations.write().await;
                    local_violations.insert(user_id.to_string(), record.clone());
                    Ok(record.violation_count)
                } else {
                    Ok(0)
                }
            } else {
                Ok(0)
            }
        }
    }

    /// Reset violation count (admin only - for testing/forgiveness)
    pub async fn reset_violations(&self, user_id: &str) -> Result<()> {
        let mut local_violations = self.local_violations.write().await;
        
        if let Some(record) = local_violations.get_mut(user_id) {
            record.violation_count = 0;
            record.ban_until = None;
            record.is_permanently_banned = false;
            
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            record.last_violation_time = now.to_string();

            // Sync with Firestore
            if let Some(client) = &self.firestore_client {
                client.update_violation_record(record).await?;
            }
        }

        Ok(())
    }
}

impl Default for ViolationTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_first_violation_allowed() {
        let tracker = ViolationTracker::new();
        let result = tracker.record_violation("user123", ViolationType::SystemPromptLeakage).await.unwrap();
        assert!(result.is_allowed);
    }

    #[tokio::test]
    async fn test_second_violation_timeout() {
        let tracker = ViolationTracker::new();
        tracker.record_violation("user456", ViolationType::SystemPromptLeakage).await.unwrap();
        let result = tracker.record_violation("user456", ViolationType::SecurityStringLeakage).await.unwrap();
        assert!(!result.is_allowed);
        assert!(result.ban_duration.is_some());
    }

    #[tokio::test]
    async fn test_third_violation_permanent_ban() {
        let tracker = ViolationTracker::new();
        tracker.record_violation("user789", ViolationType::SystemPromptLeakage).await.unwrap();
        tracker.record_violation("user789", ViolationType::SecurityStringLeakage).await.unwrap();
        let result = tracker.record_violation("user789", ViolationType::JailbreakAttempt).await.unwrap();
        assert!(!result.is_allowed);
        assert!(result.ban_duration.is_none());
    }
}
// User Management and Permissions Module
// Role-based access control with user profiles and permission checks

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// User roles ordered by privilege (higher = more privileged)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    Viewer = 0,
    Developer = 1,
    Maintainer = 2,
    Admin = 3,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "viewer" => Some(Role::Viewer),
            "developer" => Some(Role::Developer),
            "maintainer" => Some(Role::Maintainer),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Developer => "developer",
            Role::Maintainer => "maintainer",
            Role::Admin => "admin",
        }
    }
}

/// Permissions that can be granted to roles or individual users
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Project permissions
    ProjectRead,
    ProjectWrite,
    ProjectDelete,
    ProjectPublish,
    // Execution permissions
    ExecuteCode,
    ExecuteSandboxed,
    ExecutePrivileged,
    // Admin permissions
    ManageUsers,
    ManageRoles,
    ViewAuditLog,
    ManageApiKeys,
    // Training permissions
    SubmitTrainingJob,
    ManageModels,
    // System permissions
    ViewMetrics,
    ViewLogs,
    ShutdownSystem,
}

impl Permission {
    /// Default permissions granted to each role
    pub fn default_for_role(role: Role) -> HashSet<Permission> {
        use Permission::*;
        let mut perms = HashSet::new();
        match role {
            Role::Viewer => {
                perms.insert(ProjectRead);
                perms.insert(ViewMetrics);
            }
            Role::Developer => {
                perms.extend([
                    ProjectRead, ProjectWrite, ExecuteCode, ExecuteSandboxed,
                    SubmitTrainingJob, ViewMetrics, ViewLogs,
                ]);
            }
            Role::Maintainer => {
                perms.extend([
                    ProjectRead, ProjectWrite, ProjectDelete, ProjectPublish,
                    ExecuteCode, ExecuteSandboxed, SubmitTrainingJob,
                    ManageModels, ViewMetrics, ViewLogs, ViewAuditLog,
                ]);
            }
            Role::Admin => {
                // All permissions
                for p in [
                    ProjectRead, ProjectWrite, ProjectDelete, ProjectPublish,
                    ExecuteCode, ExecuteSandboxed, ExecutePrivileged,
                    ManageUsers, ManageRoles, ViewAuditLog, ManageApiKeys,
                    SubmitTrainingJob, ManageModels, ViewMetrics, ViewLogs,
                    ShutdownSystem,
                ] {
                    perms.insert(p);
                }
            }
        }
        perms
    }
}

/// A user account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub extra_permissions: HashSet<Permission>,
    pub denied_permissions: HashSet<Permission>,
    pub is_active: bool,
    pub created_at: u64,
    pub last_login_at: Option<u64>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl User {
    pub fn effective_permissions(&self) -> HashSet<Permission> {
        let mut perms = Permission::default_for_role(self.role);
        for p in &self.extra_permissions {
            perms.insert(p.clone());
        }
        for p in &self.denied_permissions {
            perms.remove(p);
        }
        perms
    }

    pub fn has_permission(&self, perm: &Permission) -> bool {
        self.is_active && self.effective_permissions().contains(perm)
    }

    pub fn has_min_role(&self, min: Role) -> bool {
        self.is_active && self.role >= min
    }
}

/// User management result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManagementResult {
    pub success: bool,
    pub message: String,
    pub user: Option<User>,
}

/// Central user manager
pub struct UserManager {
    users: Arc<RwLock<HashMap<String, User>>>, // keyed by user_id
    username_index: Arc<RwLock<HashMap<String, String>>>, // username -> user_id
}

impl UserManager {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            username_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Create a new user
    pub async fn create_user(
        &self,
        username: &str,
        email: &str,
        role: Role,
    ) -> Result<UserManagementResult> {
        if username.is_empty() || email.is_empty() {
            return Ok(UserManagementResult {
                success: false,
                message: "Username and email are required".to_string(),
                user: None,
            });
        }

        let mut index = self.username_index.write().await;
        if index.contains_key(username) {
            return Ok(UserManagementResult {
                success: false,
                message: format!("Username '{}' already exists", username),
                user: None,
            });
        }

        let user_id = format!("user_{}", uuid_simple());
        let user = User {
            user_id: user_id.clone(),
            username: username.to_string(),
            email: email.to_string(),
            role,
            extra_permissions: HashSet::new(),
            denied_permissions: HashSet::new(),
            is_active: true,
            created_at: Self::now_ms(),
            last_login_at: None,
            metadata: HashMap::new(),
        };

        index.insert(username.to_string(), user_id.clone());
        self.users.write().await.insert(user_id, user.clone());

        Ok(UserManagementResult {
            success: true,
            message: format!("User '{}' created with role {}", username, role.as_str()),
            user: Some(user),
        })
    }

    /// Get a user by ID
    pub async fn get_user(&self, user_id: &str) -> Option<User> {
        self.users.read().await.get(user_id).cloned()
    }

    /// Get a user by username
    pub async fn get_user_by_username(&self, username: &str) -> Option<User> {
        let index = self.username_index.read().await;
        if let Some(id) = index.get(username) {
            self.users.read().await.get(id).cloned()
        } else {
            None
        }
    }

    /// List all users
    pub async fn list_users(&self) -> Vec<User> {
        self.users.read().await.values().cloned().collect()
    }

    /// Update a user's role (requires caller to have checked permissions)
    pub async fn set_role(&self, user_id: &str, role: Role) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.role = role;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Activate or deactivate a user
    pub async fn set_active(&self, user_id: &str, active: bool) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.is_active = active;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Grant an extra permission to a user
    pub async fn grant_permission(&self, user_id: &str, perm: Permission) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.extra_permissions.insert(perm);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Deny a permission for a user (overrides role defaults)
    pub async fn deny_permission(&self, user_id: &str, perm: Permission) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.denied_permissions.insert(perm.clone());
            user.extra_permissions.remove(&perm);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Record a login
    pub async fn record_login(&self, user_id: &str) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.last_login_at = Some(Self::now_ms());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete a user
    pub async fn delete_user(&self, user_id: &str) -> Result<bool> {
        let mut users = self.users.write().await;
        if let Some(user) = users.remove(user_id) {
            let mut index = self.username_index.write().await;
            index.remove(&user.username);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check whether a user has a permission
    pub async fn check_permission(&self, user_id: &str, perm: &Permission) -> bool {
        if let Some(user) = self.get_user(user_id).await {
            user.has_permission(perm)
        } else {
            false
        }
    }

    /// Check whether a user meets a minimum role
    pub async fn check_role(&self, user_id: &str, min: Role) -> bool {
        if let Some(user) = self.get_user(user_id).await {
            user.has_min_role(min)
        } else {
            false
        }
    }

    /// Authorization guard: returns Err if user lacks permission
    pub async fn require_permission(&self, user_id: &str, perm: &Permission) -> Result<()> {
        if self.check_permission(user_id, perm).await {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "User '{}' lacks permission {:?}",
                user_id,
                perm
            ))
        }
    }

    pub async fn user_count(&self) -> usize {
        self.users.read().await.len()
    }
}

impl Default for UserManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple unique ID generator (time + counter based, no external deps)
fn uuid_simple() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", ts, c)
}

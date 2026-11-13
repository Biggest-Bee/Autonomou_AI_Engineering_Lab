// Native OS Hybrid Sandbox (NOSH)
// Unified SandboxEngine trait with platform-specific drivers

use async_trait::async_trait;
use std::process::Output;
use anyhow::Result;

pub mod wsl;
pub mod macos;
pub mod windows;

pub use wsl::WslSandbox;
pub use macos::MacVfkitSandbox;
pub use windows::WindowsSandbox;

/// Unified Sandbox Engine trait for cross-platform sandbox operations
#[async_trait]
pub trait SandboxEngine: Send + Sync {
    /// Boot or initialize the background sandbox VM
    async fn init(&self) -> Result<(), String>;
    
    /// Execute a command safely inside the isolated environment
    async fn exec(&self, command: &str) -> Result<Output, String>;
    
    /// Destroy or reset the sandbox instance
    async fn terminate(&self) -> Result<(), String>;
    
    /// Get sandbox status information
    async fn status(&self) -> Result<SandboxStatus, String>;
}

/// Sandbox status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct SandboxStatus {
    pub is_running: bool,
    pub platform: String,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_mb: u32,
    pub disk_mb: u32,
}

/// Factory function to create platform-specific sandbox engine
pub fn create_sandbox_engine() -> Result<Box<dyn SandboxEngine>, String> {
    #[cfg(target_os = "linux")]
    {
        WslSandbox::new("ubuntu")
            .map(|s| Box::new(s) as Box<dyn SandboxEngine>)
            .map_err(|e| e.to_string())
    }
    
    #[cfg(target_os = "macos")]
    {
        MacVfkitSandbox::new("/tmp/vm.sock")
            .map(|s| Box::new(s) as Box<dyn SandboxEngine>)
            .map_err(|e| e.to_string())
    }
    
    #[cfg(target_os = "windows")]
    {
        WindowsSandbox::new("Ubuntu")
            .map(|s| Box::new(s) as Box<dyn SandboxEngine>)
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err("Unsupported platform for sandbox engine".to_string())
    }
}
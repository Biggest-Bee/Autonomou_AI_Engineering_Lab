// Windows Sandbox Driver using WSL2
// Uses WSL2 backend for sandbox isolation

use super::{SandboxEngine, SandboxStatus, ResourceUsage};
use async_trait::async_trait;
use tokio::process::Command;
use anyhow::Result;

pub struct WindowsSandbox {
    pub distro_name: String,
}

impl WindowsSandbox {
    pub fn new(distro_name: &str) -> Result<Self> {
        Ok(Self {
            distro_name: distro_name.to_string(),
        })
    }
}

#[async_trait]
impl SandboxEngine for WindowsSandbox {
    async fn init(&self) -> Result<(), String> {
        // Ensure WSL2 is enabled and distro is running
        let status = Command::new("wsl.exe")
            .args(["-d", &self.distro_name, "--", "echo", "ready"])
            .status()
            .await
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            // Try to start the distro
            let start_status = Command::new("wsl.exe")
                .args(["-d", &self.distro_name])
                .status()
                .await
                .map_err(|e| e.to_string())?;

            if start_status.success() {
                Ok(())
            } else {
                Err("WSL2 failed to start".to_string())
            }
        }
    }

    async fn exec(&self, cmd: &str) -> Result<std::process::Output, String> {
        Command::new("wsl.exe")
            .args(["-d", &self.distro_name, "--", "sh", "-c", cmd])
            .output()
            .await
            .map_err(|e| e.to_string())
    }

    async fn terminate(&self) -> Result<(), String> {
        Command::new("wsl.exe")
            .args(["--terminate", &self.distro_name])
            .status()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn status(&self) -> Result<SandboxStatus, String> {
        // Check if distro is running
        let status = Command::new("wsl.exe")
            .args(["-l", "-v"])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let output = String::from_utf8_lossy(&status.stdout);
        let is_running = output.contains(&self.distro_name) && output.contains("Running");

        Ok(SandboxStatus {
            is_running,
            platform: "WSL2".to_string(),
            resource_usage: ResourceUsage {
                cpu_percent: 0.0,
                memory_mb: 2048,
                disk_mb: 10240,
            },
        })
    }
}
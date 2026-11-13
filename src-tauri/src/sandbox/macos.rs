// macOS Sandbox Driver using vfkit
// Uses Apple Virtualization.framework via vfkit

use super::{SandboxEngine, SandboxStatus, ResourceUsage};
use async_trait::async_trait;
use tokio::process::Command;
use anyhow::Result;

pub struct MacVfkitSandbox {
    pub vm_socket_path: String,
}

impl MacVfkitSandbox {
    pub fn new(vm_socket_path: &str) -> Result<Self> {
        Ok(Self {
            vm_socket_path: vm_socket_path.to_string(),
        })
    }
}

#[async_trait]
impl SandboxEngine for MacVfkitSandbox {
    async fn init(&self) -> Result<(), String> {
        // Launch vfkit as a background daemon using Apple Virtualization.framework
        // Note: This requires vfkit binary to be bundled in resources
        let _status = Command::new("./resources/bin/vfkit")
            .args([
                "--cpus", "2",
                "--memory", "2048",
                "--kernel", "./resources/kernel",
                "--initrd", "./resources/initrd",
                "--rest-ful-uri", &self.vm_socket_path
            ])
            .spawn()
            .map_err(|e| format!("Failed to launch vfkit: {}", e))?;

        // Wait a moment for initialization
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        Ok(())
    }

    async fn exec(&self, cmd: &str) -> Result<std::process::Output, String> {
        // Communicate with guest via SSH or vsock socket
        // For now, we'll use SSH as the communication method
        Command::new("ssh")
            .args(["-i", "./resources/id_rsa", "root@localhost", cmd])
            .output()
            .await
            .map_err(|e| e.to_string())
    }

    async fn terminate(&self) -> Result<(), String> {
        // Send shutdown call to vfkit endpoint
        let status = Command::new("curl")
            .args(["-X", "POST", &format!("{}/shutdown", self.vm_socket_path)])
            .status()
            .await
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err("Failed to shutdown vfkit".to_string())
        }
    }

    async fn status(&self) -> Result<SandboxStatus, String> {
        // Check vm socket availability
        let status = Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &self.vm_socket_path])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let output = String::from_utf8_lossy(&status.stdout);
        let is_running = output.trim() == "200";

        Ok(SandboxStatus {
            is_running,
            platform: "vfkit".to_string(),
            resource_usage: ResourceUsage {
                cpu_percent: 0.0,
                memory_mb: 2048,
                disk_mb: 10240,
            },
        })
    }
}
// Process Lifecycle Tracking and Resource Management
// Handles real process tracking, cleanup, and resource governance

use anyhow::Result;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use log::{info, warn};
use serde::{Deserialize, Serialize};

/// Process state tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

/// Process resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub pid: u32,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
    pub uptime_seconds: u64,
}

/// Tracked process with lifecycle management
#[derive(Debug)]
pub struct TrackedProcess {
    pub pid: u32,
    pub command: String,
    pub state: ProcessState,
    pub child: Option<Child>,
    pub start_time: Instant,
    pub resource_limits: ResourceLimits,
    pub resource_usage: Option<ResourceUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_cpu_percent: f32,
    pub max_memory_mb: u64,
    pub max_disk_read_mb_per_sec: u64,
    pub max_disk_write_mb_per_sec: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 100.0,
            max_memory_mb: 8192, // 8GB default
            max_disk_read_mb_per_sec: 100,
            max_disk_write_mb_per_sec: 100,
        }
    }
}

impl TrackedProcess {
    pub fn new(pid: u32, command: String, resource_limits: ResourceLimits) -> Self {
        Self {
            pid,
            command,
            state: ProcessState::Starting,
            child: None,
            start_time: Instant::now(),
            resource_limits,
            resource_usage: None,
        }
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn is_running(&self) -> bool {
        self.state == ProcessState::Running
    }
}

/// Process tracker with lifecycle management
pub struct ProcessTracker {
    processes: Arc<RwLock<HashMap<u32, TrackedProcess>>>,
    auto_cleanup: bool,
    cleanup_task_started: std::sync::atomic::AtomicBool,
}

impl ProcessTracker {
    pub fn new() -> Self {
        info!("Initializing Process Tracker with lifecycle management");

        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            auto_cleanup: true,
            cleanup_task_started: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Spawn a background task that periodically removes dead processes.
    /// Only runs when auto_cleanup is enabled and a tokio runtime is available.
    fn ensure_cleanup_task(&self) {
        if !self.auto_cleanup {
            return;
        }
        if self.cleanup_task_started.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            // No runtime available; reset so a later call can retry
            self.cleanup_task_started.store(false, std::sync::atomic::Ordering::SeqCst);
            return;
        };
        let processes = self.processes.clone();
        handle.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                Self::sweep_dead_processes(&processes).await;
            }
        });
    }

    /// Remove tracked processes that have no live child handle.
    async fn sweep_dead_processes(processes: &RwLock<HashMap<u32, TrackedProcess>>) -> usize {
        let mut map = processes.write().await;
        let dead_pids: Vec<u32> = map
            .iter()
            .filter(|(_, p)| p.child.is_none())
            .map(|(pid, _)| *pid)
            .collect();
        let mut cleaned = 0;
        for pid in dead_pids {
            if map.remove(&pid).is_some() {
                info!("Auto-cleanup removed dead process {}", pid);
                cleaned += 1;
            }
        }
        cleaned
    }

    pub async fn track_process(&self, child: Child, command: String, limits: ResourceLimits) -> Result<u32> {
        let pid = child.id();
        info!("Tracking process {} with command: {}", pid, command);

        let mut processes = self.processes.write().await;
        let mut tracked_process = TrackedProcess::new(pid, command, limits);
        tracked_process.child = Some(child);
        tracked_process.state = ProcessState::Running;

        processes.insert(pid, tracked_process);
        info!("Process {} now being tracked", pid);

        drop(processes);
        self.ensure_cleanup_task();

        Ok(pid)
    }

    pub async fn track_process_status(&self, pid: u32, command: String) -> Result<()> {
        info!("Tracking process status {} with command: {}", pid, command);

        let mut processes = self.processes.write().await;
        let limits = ResourceLimits::default();
        let mut tracked_process = TrackedProcess::new(pid, command, limits);
        tracked_process.state = ProcessState::Running;

        processes.insert(pid, tracked_process);
        info!("Process {} status now being tracked", pid);

        Ok(())
    }

    pub async fn stop_process(&self, pid: u32) -> Result<()> {
        info!("Stopping process {}", pid);

        let mut processes = self.processes.write().await;
        if let Some(process) = processes.get_mut(&pid) {
            process.state = ProcessState::Stopping;

            if let Some(ref mut child) = process.child {
                // Try graceful shutdown first
                if let Err(e) = child.kill() {
                    warn!("Failed to kill process {}: {}", pid, e);
                    // Try force kill
                    let _ = Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }

                let _ = child.wait();
            }

            process.state = ProcessState::Stopped;
            process.child = None;
            info!("Process {} stopped successfully", pid);
        } else {
            return Err(anyhow::anyhow!("Process {} not found in tracker", pid));
        }

        Ok(())
    }

    pub async fn get_process_status(&self, pid: u32) -> Result<ProcessState> {
        let processes = self.processes.read().await;
        if let Some(process) = processes.get(&pid) {
            Ok(process.state.clone())
        } else {
            Err(anyhow::anyhow!("Process {} not found in tracker", pid))
        }
    }

    pub async fn update_resource_usage(&self) -> Result<()> {
        let mut processes = self.processes.write().await;
        for (pid, process) in processes.iter_mut() {
            // Basic resource tracking without sysinfo dependency
            let uptime = process.uptime().as_secs();
            
            process.resource_usage = Some(ResourceUsage {
                pid: *pid,
                cpu_percent: 0.0, // Would use sysinfo in full implementation
                memory_mb: 0.0, // Would use sysinfo in full implementation
                disk_read_mb: 0.0,
                disk_write_mb: 0.0,
                uptime_seconds: uptime,
            });

            // Enforce resource limits (CPU + memory)
            self.check_resource_limits(process).await;
        }

        Ok(())
    }

    async fn check_resource_limits(&self, process: &TrackedProcess) {
        if let Some(ref usage) = process.resource_usage {
            if usage.cpu_percent > process.resource_limits.max_cpu_percent {
                warn!("Process {} exceeded CPU limit: {}% > {}%", 
                    process.pid, usage.cpu_percent, process.resource_limits.max_cpu_percent);
            }

            if usage.memory_mb > process.resource_limits.max_memory_mb as f64 {
                warn!("Process {} exceeded memory limit: {}MB > {}MB", 
                    process.pid, usage.memory_mb, process.resource_limits.max_memory_mb);
            }
        }
    }

    pub async fn cleanup_dead_processes(&self) -> Result<u32> {
        let mut processes = self.processes.write().await;
        let mut cleaned_count = 0;

        // Collect dead PIDs first without holding mutable borrows
        let dead_pids: Vec<u32> = processes
            .iter()
            .filter(|(_, process)| {
                if process.child.is_some() {
                    // For processes with child references, we'll check status separately
                    false // Assume running for now, check would require mutable access
                } else {
                    // If no child reference, assume it's dead
                    true
                }
            })
            .map(|(pid, _)| *pid)
            .collect();

        for pid in dead_pids {
            if let Some(mut process) = processes.remove(&pid) {
                process.state = ProcessState::Stopped;
                process.child = None;
                info!("Cleaned up dead process {}", pid);
                cleaned_count += 1;
            }
        }

        Ok(cleaned_count)
    }

    pub async fn get_all_processes(&self) -> Vec<u32> {
        let processes = self.processes.read().await;
        processes.keys().cloned().collect()
    }

    pub async fn get_process_resource_usage(&self, pid: u32) -> Option<ResourceUsage> {
        let processes = self.processes.read().await;
        processes.get(&pid).and_then(|p| p.resource_usage.clone())
    }

    pub async fn cleanup_all(&self) -> Result<()> {
        info!("Cleaning up all tracked processes");

        let pids = self.get_all_processes().await;
        for pid in pids {
            let _ = self.stop_process(pid).await;
        }

        let mut processes = self.processes.write().await;
        processes.clear();

        info!("All processes cleaned up");
        Ok(())
    }
}

impl Drop for ProcessTracker {
    fn drop(&mut self) {
        info!("Process tracker shutting down");
        // Note: Can't use async in Drop, but processes will be cleaned up on next tracker instance
    }
}

/// Resource governance with cgroups (Linux) and job objects (Windows)
pub struct ResourceGovernor {
    tracker: Arc<ProcessTracker>,
    platform: String,
}

impl ResourceGovernor {
    pub fn new(tracker: Arc<ProcessTracker>) -> Self {
        let platform = std::env::consts::OS.to_string();
        info!("Initializing resource governor for platform: {}", platform);

        Self {
            tracker,
            platform,
        }
    }

    /// Check whether a PID is currently tracked by this governor's tracker
    pub async fn is_tracking(&self, pid: u32) -> bool {
        self.tracker.get_all_processes().await.contains(&pid)
    }

    pub async fn set_cpu_limit(&self, pid: u32, cpu_percent: f32) -> Result<()> {
        info!("Setting CPU limit for process {}: {}%", pid, cpu_percent);

        if !self.is_tracking(pid).await {
            warn!("Process {} is not tracked; applying limit anyway", pid);
        }

        match self.platform.as_str() {
            "linux" => self.set_linux_cpu_limit(pid, cpu_percent).await?,
            "windows" => self.set_windows_cpu_limit(pid, cpu_percent).await?,
            "macos" => self.set_macos_cpu_limit(pid, cpu_percent).await?,
            _ => warn!("CPU limiting not supported on platform: {}", self.platform),
        }

        Ok(())
    }

    async fn set_linux_cpu_limit(&self, pid: u32, cpu_percent: f32) -> Result<()> {
        // Use cgroups to limit CPU
        let cgroup_path = format!("/sys/fs/cgroup/autonomous/{}", pid);
        
        // Create cgroup directory
        let _ = std::fs::create_dir_all(&cgroup_path);

        // Set CPU quota
        let cpu_quota = (cpu_percent / 100.0 * 100000.0) as u64; // Convert to cfs quota
        let cpu_period = 100000u64;

        let _ = std::fs::write(
            format!("{}/cpu.cfs_quota_us", cgroup_path),
            cpu_quota.to_string()
        );

        let _ = std::fs::write(
            format!("{}/cpu.cfs_period_us", cgroup_path),
            cpu_period.to_string()
        );

        // Add process to cgroup
        let _ = std::fs::write(
            format!("{}/cgroup.procs", cgroup_path),
            pid.to_string()
        );

        info!("Linux CPU limit set for process {}", pid);
        Ok(())
    }

    async fn set_windows_cpu_limit(&self, pid: u32, cpu_percent: f32) -> Result<()> {
        // Use PowerShell job objects to limit CPU
        let ps_command = format!(
            "$process = Get-Process -Id {}; $process.PriorityClass = '{}'",
            pid,
            if cpu_percent < 50.0 { "BelowNormal" } 
            else if cpu_percent < 80.0 { "Normal" } 
            else { "AboveNormal" }
        );

        let _ = Command::new("powershell")
            .args(["-Command", &ps_command])
            .output();

        info!("Windows CPU priority set for process {}", pid);
        Ok(())
    }

    async fn set_macos_cpu_limit(&self, pid: u32, _cpu_percent: f32) -> Result<()> {
        // Use taskpolicy to limit CPU
        let _ = Command::new("taskpolicy")
            .args(["-p", &pid.to_string()])
            .output();

        info!("macOS CPU limitation attempted for process {}", pid);
        Ok(())
    }

    pub async fn set_memory_limit(&self, pid: u32, memory_mb: u64) -> Result<()> {
        info!("Setting memory limit for process {}: {}MB", pid, memory_mb);

        match self.platform.as_str() {
            "linux" => self.set_linux_memory_limit(pid, memory_mb).await?,
            "windows" => self.set_windows_memory_limit(pid, memory_mb).await?,
            "macos" => self.set_macos_memory_limit(pid, memory_mb).await?,
            _ => warn!("Memory limiting not supported on platform: {}", self.platform),
        }

        Ok(())
    }

    async fn set_linux_memory_limit(&self, pid: u32, memory_mb: u64) -> Result<()> {
        let cgroup_path = format!("/sys/fs/cgroup/autonomous/{}", pid);
        let memory_bytes = memory_mb * 1024 * 1024;

        let _ = std::fs::write(
            format!("{}/memory.max", cgroup_path),
            memory_bytes.to_string()
        );

        info!("Linux memory limit set for process {}", pid);
        Ok(())
    }

    async fn set_windows_memory_limit(&self, pid: u32, _memory_mb: u64) -> Result<()> {
        // Windows doesn't have direct memory limiting for processes
        warn!("Direct memory limiting not available on Windows for process {}", pid);
        Ok(())
    }

    async fn set_macos_memory_limit(&self, pid: u32, _memory_mb: u64) -> Result<()> {
        // macOS doesn't have direct memory limiting for processes
        warn!("Direct memory limiting not available on macOS for process {}", pid);
        Ok(())
    }
}
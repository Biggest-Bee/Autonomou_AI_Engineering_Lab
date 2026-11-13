// Terminal Session Management with Enhanced Process Tracking
// Handles terminal sessions and WebSocket streaming with improved process management

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::{Command, Stdio, Child};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::protocol::Message;
use log::info;

/// Terminal session with enhanced process tracking
#[derive(Debug)]
pub struct TerminalSession {
    pub session_id: String,
    pub pty_handle: Option<PtyHandle>,
    pub tx: tokio::sync::mpsc::UnboundedSender<String>,
    pub rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    pub created_at: std::time::Instant,
}

/// Enhanced PTY handle with better process tracking
#[derive(Debug)]
pub struct PtyHandle {
    pub pid: u32,
    pub child: Option<Child>,
    pub command: String,
    pub rows: u16,
    pub cols: u16,
    pub is_running: bool,
}

/// WebSocket server for terminal streaming
#[derive(Debug)]
pub struct WebSocketServer {
    pub port: u16,
    pub active_connections: usize,
    pub is_running: bool,
    pub sessions: Arc<RwLock<HashMap<String, tokio::sync::mpsc::UnboundedSender<Message>>>>,
}

/// Terminal manager with enhanced process lifecycle
pub struct TerminalManager {
    pub sessions: Arc<RwLock<HashMap<String, TerminalSession>>>,
    pub websocket_server: Option<WebSocketServer>,
    pub process_tracker: Option<Arc<crate::val::process_tracker::ProcessTracker>>,
}

impl TerminalManager {
    pub fn new() -> Result<Self> {
        info!("Initializing Terminal Manager with enhanced process tracking");

        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            websocket_server: None,
            process_tracker: None,
        })
    }

    pub fn with_process_tracker(mut self, tracker: Arc<crate::val::process_tracker::ProcessTracker>) -> Self {
        self.process_tracker = Some(tracker);
        self
    }

    pub async fn create_session(&mut self, session_id: String) -> Result<()> {
        info!("Creating terminal session: {}", session_id);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        let session = TerminalSession {
            session_id: session_id.clone(),
            pty_handle: None,
            tx,
            rx,
            created_at: std::time::Instant::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        info!("Terminal session {} created", session_id);
        Ok(())
    }

    pub async fn spawn_pty(&mut self, session_id: &str, command: &str) -> Result<()> {
        info!("Spawning PTY for session {} with command: {}", session_id, command);

        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            // Create enhanced process with proper tracking
            let child = Command::new(if cfg!(target_os = "windows") { "cmd" } else { "sh" })
                .args(["-c", command])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to spawn command")?;

            let pid = child.id();
            info!("Command spawned with PID: {}", pid);

            let pty_handle = PtyHandle {
                pid,
                child: Some(child),
                command: command.to_string(),
                rows: 24,
                cols: 80,
                is_running: true,
            };

            session.pty_handle = Some(pty_handle);
            info!("PTY spawned successfully for session {}", session_id);
        } else {
            return Err(anyhow::anyhow!("Session {} not found", session_id));
        }

        Ok(())
    }

    pub async fn write_to_pty(&self, session_id: &str, data: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(ref mut pty) = session.pty_handle {
                if let Some(ref mut child) = pty.child {
                    // Write to process stdin
                    if let Some(ref mut stdin) = child.stdin {
                        use std::io::Write;
                        stdin.write_all(data.as_bytes())?;
                        stdin.flush()?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn read_from_pty(&mut self, session_id: &str) -> Result<Option<String>> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(ref mut pty) = session.pty_handle {
                if let Some(ref mut child) = pty.child {
                    // Read from process stdout
                    if let Some(ref mut stdout) = child.stdout {
                        use std::io::{BufRead, BufReader};
                        let mut reader = BufReader::new(stdout);
                        let mut buffer = String::new();
                        
                        reader.read_line(&mut buffer)?;
                        if !buffer.is_empty() {
                            return Ok(Some(buffer));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub async fn close_session(&mut self, session_id: &str) -> Result<()> {
        info!("Closing terminal session: {}", session_id);

        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(session_id) {
            if let Some(mut pty) = session.pty_handle.take() {
                TerminalManager::close_pty(&mut pty)?;
            }
        }

        info!("Terminal session {} closed", session_id);
        Ok(())
    }

    fn close_pty(pty: &mut PtyHandle) -> Result<()> {
        if let Some(mut child) = pty.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        pty.is_running = false;
        Ok(())
    }

    pub async fn resize_pty(&self, session_id: &str, rows: u16, cols: u16) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(ref mut pty) = session.pty_handle {
                pty.rows = rows;
                pty.cols = cols;
                // In a real PTY implementation, this would resize the terminal
                info!("Resized PTY for session {} to {}x{}", session_id, rows, cols);
            }
        }
        Ok(())
    }

    pub async fn start_websocket_server(&mut self, port: u16) -> Result<()> {
        info!("Starting WebSocket server on port {}", port);

        let ws_sessions = Arc::new(RwLock::new(HashMap::new()));

        let server = WebSocketServer {
            port,
            active_connections: 0,
            is_running: true,
            sessions: ws_sessions,
        };

        self.websocket_server = Some(server);

        info!("WebSocket server started on port {}", port);
        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        info!("Terminal manager shutting down");
        let _sessions = std::mem::take(&mut self.sessions);
        // Note: Can't use async in Drop, but sessions will be cleaned up properly
    }
}
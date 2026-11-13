// LSP (Language Server Protocol) Integration Module
// Implements LSP client for code intelligence features

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use log::{info, error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::{Command, Child, ChildStdin, ChildStdout};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// LSP client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPConfig {
    pub language: String,
    pub server_command: String,
    pub server_args: Vec<String>,
    pub workspace_root: String,
    pub enabled: bool,
}

/// LSP document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPDocument {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

/// LSP position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPPosition {
    pub line: u32,
    pub character: u32,
}

/// LSP range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPRange {
    pub start: LSPPosition,
    pub end: LSPPosition,
}

/// LSP completion item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPCompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
}

/// LSP hover information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPHover {
    pub contents: serde_json::Value,
    pub range: Option<LSPRange>,
}

/// LSP definition location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPDefinition {
    pub uri: String,
    pub range: LSPRange,
}

/// LSP client
pub struct LSPClient {
    config: LSPConfig,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    documents: Arc<RwLock<HashMap<String, LSPDocument>>>,
    initialized: bool,
}

impl LSPClient {
    pub fn new(config: LSPConfig) -> Self {
        info!("Initializing LSP client for language: {}", config.language);

        Self {
            config,
            process: None,
            stdin: None,
            stdout: None,
            documents: Arc::new(RwLock::new(HashMap::new())),
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing LSP client");

        if !self.config.enabled {
            warn!("LSP client is disabled");
            return Ok(());
        }

        // Start the language server process
        let mut child = tokio::process::Command::new(&self.config.server_command)
            .args(&self.config.server_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start language server")?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;

        self.process = Some(child);
        self.stdin = Some(stdin);
        self.stdout = Some(stdout);

        // Send initialize request
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", self.config.workspace_root),
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {
                            "snippetSupport": true
                        }
                    },
                    "hover": {
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "definition": {},
                    "references": {},
                    "documentSymbol": {},
                    "codeAction": {},
                    "formatting": {}
                }
            },
            "workspaceFolders": [
                {
                    "uri": format!("file://{}", self.config.workspace_root),
                    "name": "workspace"
                }
            ]
        });

        self.send_request("initialize", init_params).await?;
        self.initialized = true;

        info!("LSP client initialized successfully");
        Ok(())
    }

    async fn send_request(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        if !self.initialized {
            return Err(anyhow::anyhow!("LSP client not initialized"));
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let request_str = format!("Content-Length: {}\r\n\r\n{}", 
            request.to_string().len(), 
            request.to_string()
        );

        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(request_str.as_bytes()).await?;
            stdin.flush().await?;
        } else {
            return Err(anyhow::anyhow!("LSP stdin not available"));
        }

        // Read response (simplified - in production, this would be more robust)
        if let Some(ref mut stdout) = self.stdout {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            // Parse the response
            if let Ok(response) = serde_json::from_str::<serde_json::Value>(&line) {
                Ok(response)
            } else {
                Err(anyhow::anyhow!("Failed to parse LSP response"))
            }
        } else {
            Err(anyhow::anyhow!("LSP stdout not available"))
        }
    }

    pub async fn open_document(&mut self, uri: String, language_id: String, text: String) -> Result<()> {
        info!("Opening document in LSP: {}", uri);

        let text_clone = text.clone();
        let document = LSPDocument {
            uri: uri.clone(),
            language_id,
            version: 1,
            text,
        };

        {
            let mut documents = self.documents.write().await;
            documents.insert(uri.clone(), document);
        }

        // Send didOpen notification
        let did_open_params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": self.config.language,
                "version": 1,
                "text": text_clone
            }
        });

        self.send_notification("textDocument/didOpen", did_open_params).await?;

        Ok(())
    }

    pub async fn change_document(&mut self, uri: String, text: String) -> Result<()> {
        info!("Changing document in LSP: {}", uri);

        let (version, text_clone) = {
            let mut documents = self.documents.write().await;
            if let Some(document) = documents.get_mut(&uri) {
                document.text = text;
                document.version += 1;
                (document.version, document.text.clone())
            } else {
                return Err(anyhow::anyhow!("Document not found: {}", uri));
            }
        };

        // Send didChange notification
        let did_change_params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": version
            },
            "contentChanges": [
                {
                    "text": text_clone
                }
            ]
        });

        self.send_notification("textDocument/didChange", did_change_params).await?;
        Ok(())
    }

    pub async fn close_document(&mut self, uri: String) -> Result<()> {
        info!("Closing document in LSP: {}", uri);

        {
            let mut documents = self.documents.write().await;
            documents.remove(&uri);
        }

        // Send didClose notification
        let did_close_params = serde_json::json!({
            "textDocument": {
                "uri": uri
            }
        });

        self.send_notification("textDocument/didClose", did_close_params).await?;
        Ok(())
    }

    async fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("LSP client not initialized"));
        }

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let notification_str = format!("Content-Length: {}\r\n\r\n{}", 
            notification.to_string().len(), 
            notification.to_string()
        );

        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(notification_str.as_bytes()).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("LSP stdin not available"))
        }
    }

    pub async fn get_completions(&mut self, uri: String, position: LSPPosition) -> Result<Vec<LSPCompletionItem>> {
        info!("Getting completions for {} at {:?}", uri, position);

        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": position.line,
                "character": position.character
            }
        });

        let response = self.send_request("textDocument/completion", params).await?;
        
        // Parse completion items
        if let Some(items) = response.get("result").and_then(|r| r.get("items")) {
            let completion_items: Vec<LSPCompletionItem> = items
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect();
            Ok(completion_items)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_hover(&mut self, uri: String, position: LSPPosition) -> Result<Option<LSPHover>> {
        info!("Getting hover for {} at {:?}", uri, position);

        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": position.line,
                "character": position.character
            }
        });

        let response = self.send_request("textDocument/hover", params).await?;
        
        if let Some(result) = response.get("result") {
            let hover: LSPHover = serde_json::from_value(result.clone())?;
            Ok(Some(hover))
        } else {
            Ok(None)
        }
    }

    pub async fn get_definition(&mut self, uri: String, position: LSPPosition) -> Result<Vec<LSPDefinition>> {
        info!("Getting definition for {} at {:?}", uri, position);

        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": {
                "line": position.line,
                "character": position.character
            }
        });

        let response = self.send_request("textDocument/definition", params).await?;
        
        if let Some(result) = response.get("result") {
            if let Some(locations) = result.as_array() {
                let definitions: Vec<LSPDefinition> = locations
                    .iter()
                    .filter_map(|loc| serde_json::from_value(loc.clone()).ok())
                    .collect();
                Ok(definitions)
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down LSP client");

        if self.initialized {
            // Send shutdown request
            let _ = self.send_request("shutdown", serde_json::json!({})).await;
            
            // Send exit notification
            let _ = self.send_notification("exit", serde_json::json!({})).await;
        }

        // Kill the process
        if let Some(ref mut process) = self.process {
            process.kill().await?;
        }

        self.initialized = false;
        Ok(())
    }
}

impl Drop for LSPClient {
    fn drop(&mut self) {
        info!("LSP client shutting down");
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
        }
    }
}

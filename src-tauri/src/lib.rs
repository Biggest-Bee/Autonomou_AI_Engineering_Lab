pub mod val;
pub mod security;
pub mod sandbox;
pub mod firebase;
pub mod cas;
pub mod marketplace;
pub mod cloud_training;
pub mod execution_graph;
pub mod autonomy;
pub mod verification;
pub mod mock_api;
pub mod lsp;
pub mod logging;
pub mod performance;
pub mod user_mgmt;
pub mod audit;
pub mod i18n;

use val::{ValDriver, NetworkStack, TerminalManager};
use security::{SecurityConfig, SecurityFilter, RateLimiter, ViolationTracker, MessageContainer};
use sandbox::SandboxEngine;
use tokio::sync::Mutex;
use tauri::{State, Manager};
use std::process::Stdio;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct IpcMessage {
    #[serde(rename = "type")]
    message_type: String,
    file_path: Option<String>,
    language: Option<String>,
    code: Option<String>,
    query: Option<String>,
    symbol_name: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IpcResponse {
    status: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

struct AppState {
    val_driver: Mutex<Option<ValDriver>>,
    network_stack: Mutex<Option<NetworkStack>>,
    terminal_manager: Mutex<Option<TerminalManager>>,
    python_daemon: Arc<Mutex<Option<PythonDaemon>>>,
    security_config: SecurityConfig,
    security_filter: Arc<SecurityFilter>,
    rate_limiter: Arc<RateLimiter>,
    violation_tracker: Arc<ViolationTracker>,
    sandbox_engine: Arc<Mutex<Option<Box<dyn SandboxEngine>>>>,
    firestore_client: Arc<Mutex<Option<firebase::firestore::FirestoreClient>>>,
    cas_storage: Arc<Mutex<Option<cas::ContentAddressableStorage>>>,
    project_manager: Arc<Mutex<Option<marketplace::ProjectManager>>>,
    cloud_training_manager: Arc<Mutex<Option<cloud_training::CloudTrainingManager>>>,
    execution_graph_manager: Arc<Mutex<Option<execution_graph::ExecutionGraphManager>>>,
    autonomy_calculator: Arc<Mutex<Option<autonomy::AutonomyCalculator>>>,
    verification_manager: Arc<Mutex<Option<verification::VerificationManager>>>,
    mock_api_service: Arc<Mutex<Option<mock_api::MockAPIService>>>,
    lsp_clients: Arc<Mutex<HashMap<String, lsp::LSPClient>>>,
    logging_manager: Arc<logging::LoggingManager>,
    performance_monitor: Arc<performance::PerformanceMonitor>,
    user_manager: Arc<user_mgmt::UserManager>,
    audit_logger: Arc<audit::AuditLogger>,
    language_manager: Arc<i18n::LanguageManager>,
}

struct PythonDaemon {
    child: Option<tokio::process::Child>,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
}

impl PythonDaemon {
    async fn new() -> Result<Self, anyhow::Error> {
        log::info!("Starting Python daemon");
        
        let python_path = if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        };
        
        let daemon_path = std::path::Path::new("python-daemon/main.py");
        if !daemon_path.exists() {
            anyhow::bail!("Python daemon not found at {:?}", daemon_path);
        }
        
        let mut child = tokio::process::Command::new(python_path)
            .arg(daemon_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        
        let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
        
        log::info!("Python daemon started with PID: {:?}", child.id());
        
        Ok(PythonDaemon {
            child: Some(child),
            stdin: Some(stdin),
            stdout: Some(stdout),
        })
    }
    
    async fn send_message(&mut self, message: &IpcMessage) -> Result<(), anyhow::Error> {
        if let Some(ref mut stdin) = self.stdin {
            let json = serde_json::to_string(message)?;
            stdin.write_all(json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
            Ok(())
        } else {
            anyhow::bail!("stdin not available")
        }
    }
    
    async fn receive_response(&mut self) -> Result<IpcResponse, anyhow::Error> {
        if let Some(ref mut stdout) = self.stdout {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            if line.trim().is_empty() {
                // Wait for response
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                reader.read_line(&mut line).await?;
            }
            
            let response: IpcResponse = serde_json::from_str(&line.trim())?;
            Ok(response)
        } else {
            anyhow::bail!("stdout not available")
        }
    }
    
    async fn shutdown(&mut self) {
        log::info!("Shutting down Python daemon");
        
        // Send shutdown message
        let _ = self.send_message(&IpcMessage {
            message_type: "shutdown".to_string(),
            file_path: None,
            language: None,
            code: None,
            query: None,
            symbol_name: None,
            line: None,
            column: None,
            limit: None,
        }).await;
        
        // Kill the process
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        
        self.stdin = None;
        self.stdout = None;
    }
}

// Drop kills the daemon child process so it doesn't leak when the app exits.
// Uses the synchronous start_kill() since Drop can't be async.
impl Drop for PythonDaemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      // Load environment variables from .env file
      let _ = dotenv::dotenv();
      
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      // Initialize VAL (Virtualization Abstraction Layer) drivers
      let val_driver = ValDriver::new().expect("Failed to initialize VAL driver");
      val_driver.initialize().expect("Failed to initialize VAL driver");
      
      // Initialize Network Stack
      let network_stack = NetworkStack::new().expect("Failed to initialize network stack");
      
      // Configure platform-specific network routing
      #[cfg(target_os = "linux")]
      {
        let _ = network_stack.configure_ebpf_routing();
      }
      
      #[cfg(target_os = "macos")]
      {
        let _ = network_stack.configure_vmnet_routing();
      }
      
      #[cfg(target_os = "windows")]
      {
        let _ = network_stack.configure_hyper_v_routing();
      }
      
      // Initialize Terminal Manager
      let terminal_manager = TerminalManager::new().expect("Failed to initialize terminal manager");
      
      // Initialize security components
      let security_config = SecurityConfig::from_env().expect("Failed to load security config");
      let security_filter = Arc::new(SecurityFilter::new(security_config.clone()));
      let rate_limiter = Arc::new(RateLimiter::new());
      let violation_tracker = Arc::new(ViolationTracker::new());
      
      // Initialize sandbox engine (deferred initialization)
      let sandbox_engine = sandbox::create_sandbox_engine().expect("Failed to create sandbox engine");
      
      // Initialize Stage 3 components
      let firestore_client = Arc::new(Mutex::new(None));
      let cas_storage = Arc::new(Mutex::new(None));
      let project_manager = Arc::new(Mutex::new(None));
      let cloud_training_manager = Arc::new(Mutex::new(None));
      
      // Initialize Stage 4 components
      let execution_graph_manager = Arc::new(Mutex::new(None));
      let autonomy_calculator = Arc::new(Mutex::new(None));
      let verification_manager = Arc::new(Mutex::new(None));
      let mock_api_service = Arc::new(Mutex::new(None));
      let lsp_clients = Arc::new(Mutex::new(HashMap::new()));

      // Initialize Stage 5 components
      let logging_manager = Arc::new(logging::LoggingManager::default());
      let performance_monitor = Arc::new(performance::PerformanceMonitor::default());
      let user_manager = Arc::new(user_mgmt::UserManager::new());
      let audit_logger = Arc::new(audit::AuditLogger::default());
      let language_manager = Arc::new(i18n::LanguageManager::new());
      // Language defaults are populated asynchronously after setup
      let lm_init = language_manager.clone();
      tauri::async_runtime::spawn(async move {
        let _ = lm_init.initialize().await;
      });

      // Store in app state
      app.manage(AppState {
        val_driver: Mutex::new(Some(val_driver)),
        network_stack: Mutex::new(Some(network_stack)),
        terminal_manager: Mutex::new(Some(terminal_manager)),
        python_daemon: Arc::new(Mutex::new(None)), // Will be initialized later
        security_config,
        security_filter,
        rate_limiter,
        violation_tracker,
        sandbox_engine: Arc::new(Mutex::new(Some(sandbox_engine))),
        firestore_client,
        cas_storage,
        project_manager,
        cloud_training_manager,
        execution_graph_manager,
        autonomy_calculator,
        verification_manager,
        mock_api_service,
        lsp_clients,
        logging_manager,
        performance_monitor,
        user_manager,
        audit_logger,
        language_manager,
      });
      
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      greet,
      get_system_info,
      start_terminal_session,
      create_microvm,
      resolve_domain,
      configure_chaos,
      start_python_daemon,
      parse_ast,
      search_code,
      get_symbol_info,
      get_daemon_stats,
      check_security,
      format_message,
      check_rate_limit,
      run_sandbox_command,
      get_sandbox_status,
      run_all_tests,
      get_production_mode,
      set_production_mode,
      initialize_firestore,
      firestore_get_document,
      firestore_set_document,
      initialize_cas,
      cas_store_content,
      cas_retrieve_content,
      initialize_project_manager,
      create_project,
      publish_to_marketplace,
      search_marketplace,
      initialize_cloud_training,
      submit_training_job,
      start_training_job,
      get_training_job_status,
      list_training_jobs,
      initialize_execution_graph,
      create_execution_graph,
      add_graph_node,
      add_graph_edge,
      get_graph_visualization,
      get_execution_order,
      get_graph_stats,
      initialize_autonomy_calculator,
      start_autonomy_session,
      record_autonomous_action,
      record_self_correction,
      record_human_intervention,
      record_task_completion,
      calculate_autonomy_score,
      get_autonomy_score,
      get_autonomy_leaderboard,
      initialize_verification_manager,
      run_system_verification,
      run_performance_verification,
      get_verification_report,
      get_all_verification_reports,
      initialize_mock_api,
      register_mock_endpoint,
      handle_mock_request,
      get_mock_api_stats,
      initialize_lsp_client,
      lsp_open_document,
      lsp_change_document,
      lsp_get_completions,
      lsp_get_hover,
      lsp_get_definition,
      lsp_shutdown,
      log_message,
      query_logs,
      get_monitoring_stats,
      record_metric,
      get_performance_aggregates,
      run_benchmark_command,
      create_user,
      list_users,
      get_user,
      set_user_role,
      check_user_permission,
      record_audit_event,
      query_audit_log,
      verify_audit_chain,
      get_audit_stats,
      detect_file_language,
      list_languages,
      translate_string,
      set_locale
    ])
    .on_window_event(|window, event| {
      if let tauri::WindowEvent::CloseRequested { .. } = event {
        // Gracefully shut down the Python daemon before the process exits.
        // Drop on PythonDaemon is the backstop if this doesn't finish in time.
        let daemon_mutex = window.state::<AppState>().python_daemon.clone();
        tauri::async_runtime::spawn(async move {
          let mut guard = daemon_mutex.lock().await;
          if let Some(ref mut daemon) = *guard {
            daemon.shutdown().await;
          }
        });
      }
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_system_info(state: State<'_, AppState>) -> Result<String, String> {
    let os_info = os_info::get();
    let val_info = state.val_driver.lock().await;
    
    Ok(format!(
        "OS: {} {}, Version: {}, Architecture: {}, VAL Driver: Initialized",
        os_info.os_type(),
        os_info.version(),
        std::env::consts::ARCH,
        if val_info.is_some() { "Yes" } else { "No" }
    ))
}

#[tauri::command]
async fn start_terminal_session(session_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Starting terminal session: {}", session_id);
    
    // Determine terminal type based on production mode
    let terminal_type = if state.security_config.production_mode {
        &state.security_config.default_terminal
    } else {
        // In development mode, use standard terminal
        if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "bash"
        }
    };
    
    log::info!("Using terminal type: {} (production mode: {})", terminal_type, state.security_config.production_mode);
    
    // First create the session
    {
        let mut terminal_manager = state.terminal_manager.lock().await;
        if let Some(ref mut tm) = *terminal_manager {
            tm.create_session(session_id.clone()).await
                .map_err(|e| format!("Failed to create session: {}", e))?;
        } else {
            return Err("Terminal manager not initialized".to_string());
        }
    }
    
    // Then start WebSocket server if needed
    {
        let mut terminal_manager = state.terminal_manager.lock().await;
        if let Some(ref mut tm) = *terminal_manager {
            if tm.websocket_server.is_none() {
                tm.start_websocket_server(8765).await
                    .map_err(|e| format!("Failed to start WebSocket server: {}", e))?;
            }
        }
    }
    
    Ok(format!("Terminal session {} started with {}", session_id, terminal_type))
}

#[tauri::command]
async fn start_python_daemon(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Starting Python daemon");
    
    let mut python_daemon = state.python_daemon.lock().await;
    if python_daemon.is_some() {
        return Err("Python daemon already running".to_string());
    }
    
    let daemon = PythonDaemon::new().await
        .map_err(|e| format!("Failed to start Python daemon: {}", e))?;
    
    *python_daemon = Some(daemon);
    
    Ok("Python daemon started successfully".to_string())
}

#[tauri::command]
async fn create_microvm(cpu_count: u32, memory_mb: u32, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Creating MicroVM with {} CPUs and {} MB RAM", cpu_count, memory_mb);
    
    let val_driver = state.val_driver.lock().await;
    if let Some(ref driver) = *val_driver {
        let config = val::MicroVMConfig {
            cpu_count,
            memory_mb,
            boot_source: "/dev/null".to_string(),
            network_interfaces: vec![],
        };
        
        driver.create_microvm(config)
            .map_err(|e| format!("Failed to create MicroVM: {}", e))
    } else {
        Err("VAL driver not initialized".to_string())
    }
}

#[tauri::command]
async fn resolve_domain(domain: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Resolving domain: {}", domain);
    
    let network_stack = state.network_stack.lock().await;
    if let Some(ref stack) = *network_stack {
        stack.resolve_domain(&domain)
            .map_err(|e| format!("Failed to resolve domain: {}", e))
    } else {
        Err("Network stack not initialized".to_string())
    }
}

#[tauri::command]
async fn configure_chaos(latency_ms: u64, packet_loss: u8, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Configuring chaos: latency={}ms, packet_loss={}%", latency_ms, packet_loss);
    
    let network_stack = state.network_stack.lock().await;
    if let Some(ref stack) = *network_stack {
        let config = val::ChaosConfig {
            latency_ms,
            packet_loss_percentage: packet_loss,
            bandwidth_kbps: None,
        };
        
        stack.configure_chaos_engineering(config)
            .map_err(|e| format!("Failed to configure chaos: {}", e))
            .map(|_| "Chaos engineering configured successfully".to_string())
    } else {
        Err("Network stack not initialized".to_string())
    }
}

#[tauri::command]
async fn parse_ast(file_path: String, language: String, code: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Parsing AST: file={}, language={}", file_path, language);
    
    let mut python_daemon = state.python_daemon.lock().await;
    if let Some(ref mut daemon) = *python_daemon {
        let message = IpcMessage {
            message_type: "ast_update".to_string(),
            file_path: Some(file_path),
            language: Some(language),
            code: Some(code),
            query: None,
            symbol_name: None,
            line: None,
            column: None,
            limit: None,
        };
        
        daemon.send_message(&message).await
            .map_err(|e| format!("Failed to send message: {}", e))?;
        
        let response = daemon.receive_response().await
            .map_err(|e| format!("Failed to receive response: {}", e))?;
        
        serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    } else {
        Err("Python daemon not initialized. Call start_python_daemon first.".to_string())
    }
}

#[tauri::command]
async fn search_code(query: String, language: Option<String>, limit: Option<u32>, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Searching code: query={}, language={:?}", query, language);
    
    let mut python_daemon = state.python_daemon.lock().await;
    if let Some(ref mut daemon) = *python_daemon {
        let message = IpcMessage {
            message_type: "search".to_string(),
            file_path: None,
            language: language,
            code: None,
            query: Some(query),
            symbol_name: None,
            line: None,
            column: None,
            limit: limit,
        };
        
        daemon.send_message(&message).await
            .map_err(|e| format!("Failed to send message: {}", e))?;
        
        let response = daemon.receive_response().await
            .map_err(|e| format!("Failed to receive response: {}", e))?;
        
        serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    } else {
        Err("Python daemon not initialized. Call start_python_daemon first.".to_string())
    }
}

#[tauri::command]
async fn get_symbol_info(symbol_name: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting symbol info: symbol={}", symbol_name);
    
    let mut python_daemon = state.python_daemon.lock().await;
    if let Some(ref mut daemon) = *python_daemon {
        let message = IpcMessage {
            message_type: "symbol_info".to_string(),
            file_path: None,
            language: None,
            code: None,
            query: None,
            symbol_name: Some(symbol_name),
            line: None,
            column: None,
            limit: None,
        };
        
        daemon.send_message(&message).await
            .map_err(|e| format!("Failed to send message: {}", e))?;
        
        let response = daemon.receive_response().await
            .map_err(|e| format!("Failed to receive response: {}", e))?;
        
        serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    } else {
        Err("Python daemon not initialized. Call start_python_daemon first.".to_string())
    }
}

#[tauri::command]
async fn get_daemon_stats(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting daemon stats");
    
    let mut python_daemon = state.python_daemon.lock().await;
    if let Some(ref mut daemon) = *python_daemon {
        let message = IpcMessage {
            message_type: "stats".to_string(),
            file_path: None,
            language: None,
            code: None,
            query: None,
            symbol_name: None,
            line: None,
            column: None,
            limit: None,
        };
        
        daemon.send_message(&message).await
            .map_err(|e| format!("Failed to send message: {}", e))?;
        
        let response = daemon.receive_response().await
            .map_err(|e| format!("Failed to receive response: {}", e))?;
        
        serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    } else {
        Err("Python daemon not initialized. Call start_python_daemon first.".to_string())
    }
}

#[tauri::command]
async fn check_security(user_content: String, user_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Checking security for user: {}", user_id);
    
    // Check if user is allowed (not banned)
    let violation_check = state.violation_tracker.check_user_allowed(&user_id).await
        .map_err(|e| format!("Violation check failed: {}", e))?;
    
    if !violation_check.is_allowed {
        return Ok(serde_json::json!({
            "status": "blocked",
            "error": violation_check.error_message,
            "ban_duration": violation_check.ban_duration.map(|d| d.as_secs()).unwrap_or(0)
        }).to_string());
    }
    
    // Filter user message
    let filtered_content = state.security_filter.filter_user_message(&user_content)
        .map_err(|e| format!("Security filter failed: {}", e))?;
    
    Ok(serde_json::json!({
        "status": "allowed",
        "filtered_content": filtered_content
    }).to_string())
}

#[tauri::command]
async fn format_message(user_content: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Formatting user message");
    
    let mut message_container = MessageContainer::new();
    message_container.add_system(state.security_config.system_prompt.clone());
    message_container.add_user(user_content);
    
    message_container.to_json()
        .map_err(|e| format!("Failed to format message: {}", e))
}

#[tauri::command]
async fn check_rate_limit(request_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Checking rate limit for: {}", request_type);
    
    let req_type = match request_type.as_str() {
        "search" => security::RequestType::Search,
        "primary" => security::RequestType::Primary,
        "secondary" => security::RequestType::Secondary,
        "content_creation" => security::RequestType::ContentCreation,
        _ => return Err("Invalid request type".to_string()),
    };
    
    let result = state.rate_limiter.check_rate_limit(req_type).await
        .map_err(|e| format!("Rate limit check failed: {}", e))?;
    
    if result.allowed {
        state.rate_limiter.record_request(req_type).await;
    }
    
    Ok(serde_json::json!({
        "allowed": result.allowed,
        "retry_after": result.retry_after.map(|d| d.as_secs()),
        "reason": result.reason
    }).to_string())
}

#[tauri::command]
async fn run_sandbox_command(command: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Running sandbox command: {}", command);
    
    let sandbox = state.sandbox_engine.lock().await;
    if let Some(ref sandbox) = *sandbox {
        let output = sandbox.exec(&command).await
            .map_err(|e| format!("Sandbox command failed: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        if output.status.success() {
            Ok(serde_json::json!({
                "status": "success",
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": output.status.code()
            }).to_string())
        } else {
            Ok(serde_json::json!({
                "status": "error",
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": output.status.code()
            }).to_string())
        }
    } else {
        Err("Sandbox engine not initialized".to_string())
    }
}

#[tauri::command]
async fn get_sandbox_status(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting sandbox status");
    
    let sandbox = state.sandbox_engine.lock().await;
    if let Some(ref sandbox) = *sandbox {
        let status = sandbox.status().await
            .map_err(|e| format!("Failed to get sandbox status: {}", e))?;
        
        Ok(serde_json::to_string(&status).map_err(|e| e.to_string())?)
    } else {
        Err("Sandbox engine not initialized".to_string())
    }
}

#[tauri::command]
async fn run_all_tests() -> Result<String, String> {
    log::info!("Running all tests concurrently");
    
    // Run all test frameworks concurrently
    let (cargo_result, pytest_result, vitest_result) = tokio::join!(
        async {
            tokio::process::Command::new("cargo")
                .args(&["test"])
                .output()
                .await
        },
        async {
            tokio::process::Command::new("pytest")
                .args(&["-v"])
                .output()
                .await
        },
        async {
            tokio::process::Command::new("vitest")
                .args(&["run"])
                .output()
                .await
        }
    );
    
    let cargo_status: Result<(bool, String), std::io::Error> = cargo_result.map(|o| (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()));
    let pytest_status: Result<(bool, String), std::io::Error> = pytest_result.map(|o| (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()));
    let vitest_status: Result<(bool, String), std::io::Error> = vitest_result.map(|o| (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()));
    
    Ok(serde_json::json!({
        "cargo_test": {
            "success": cargo_status.as_ref().map(|(s, _)| *s).unwrap_or(false),
            "output": cargo_status.as_ref().map(|(_, o)| o.clone()).unwrap_or_else(|_| "Failed to run".to_string())
        },
        "pytest": {
            "success": pytest_status.as_ref().map(|(s, _)| *s).unwrap_or(false),
            "output": pytest_status.as_ref().map(|(_, o)| o.clone()).unwrap_or_else(|_| "Failed to run".to_string())
        },
        "vitest": {
            "success": vitest_status.as_ref().map(|(s, _)| *s).unwrap_or(false),
            "output": vitest_status.as_ref().map(|(_, o)| o.clone()).unwrap_or_else(|_| "Failed to run".to_string())
        }
    }).to_string())
}

#[tauri::command]
async fn get_production_mode(state: State<'_, AppState>) -> Result<String, String> {
    Ok(serde_json::json!({
        "production_mode": state.security_config.production_mode,
        "default_terminal": state.security_config.default_terminal
    }).to_string())
}

#[tauri::command]
async fn set_production_mode(enabled: bool, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Setting production mode to: {}", enabled);
    
    // Note: This would typically require restarting the app or reconfiguring
    // For now, we return the current status
    Ok(serde_json::json!({
        "status": "production_mode_change_requires_restart",
        "current_mode": state.security_config.production_mode,
        "requested_mode": enabled,
        "message": "Production mode changes require application restart. Please set PRODUCTION_MODE environment variable and restart."
    }).to_string())
}

// Stage 3: Firebase and Firestore Commands

#[tauri::command]
async fn initialize_firestore(project_id: String, credentials: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Firestore client");
    
    let client = firebase::firestore::FirestoreClient::new(project_id, credentials);
    
    match client.initialize().await {
        Ok(_) => {
            let mut firestore_guard = state.firestore_client.lock().await;
            *firestore_guard = Some(client);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Firestore client initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Firestore: {}", e))
        }
    }
}

#[tauri::command]
async fn firestore_get_document(collection: String, document_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting Firestore document: {}/{}", collection, document_id);
    
    let firestore_guard = state.firestore_client.lock().await;
    if let Some(ref client) = *firestore_guard {
        match client.get_document(&collection, &document_id).await {
            Ok(doc) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "document": doc
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get document: {}", e))
            }
        }
    } else {
        Err("Firestore client not initialized".to_string())
    }
}

#[tauri::command]
async fn firestore_set_document(collection: String, document_id: String, data: serde_json::Value, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Setting Firestore document: {}/{}", collection, document_id);
    
    let firestore_guard = state.firestore_client.lock().await;
    if let Some(ref client) = *firestore_guard {
        match client.set_document(&collection, &document_id, data).await {
            Ok(doc) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "document": doc
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to set document: {}", e))
            }
        }
    } else {
        Err("Firestore client not initialized".to_string())
    }
}

// Stage 3: CAS Commands

#[tauri::command]
async fn initialize_cas(storage_path: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing CAS storage at: {}", storage_path);
    
    let cas_path = std::path::PathBuf::from(storage_path);
    let cas = match cas::ContentAddressableStorage::new(cas_path) {
        Ok(cas) => cas,
        Err(e) => return Err(format!("Failed to create CAS: {}", e)),
    };
    
    match cas.initialize().await {
        Ok(_) => {
            let mut cas_guard = state.cas_storage.lock().await;
            *cas_guard = Some(cas);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "CAS storage initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize CAS: {}", e))
        }
    }
}

#[tauri::command]
async fn cas_store_content(content: Vec<u8>, content_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Storing content in CAS, size: {} bytes", content.len());
    
    let cas_guard = state.cas_storage.lock().await;
    if let Some(ref cas) = *cas_guard {
        match cas.store_content(&content, &content_type).await {
            Ok(address) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "address": address
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to store content: {}", e))
            }
        }
    } else {
        Err("CAS storage not initialized".to_string())
    }
}

#[tauri::command]
async fn cas_retrieve_content(hash: String, size: u64, content_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Retrieving content from CAS: {}", hash);
    
    let cas_guard = state.cas_storage.lock().await;
    if let Some(ref cas) = *cas_guard {
        let address = cas::ContentAddress {
            hash,
            size,
            content_type,
        };
        
        match cas.retrieve_content(&address).await {
            Ok(content) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "content": content,
                    "size": content.len()
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to retrieve content: {}", e))
            }
        }
    } else {
        Err("CAS storage not initialized".to_string())
    }
}

// Stage 3: Marketplace Commands

#[tauri::command]
async fn initialize_project_manager(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Project Manager");
    
    let manager = marketplace::ProjectManager::new();
    
    match manager.initialize().await {
        Ok(_) => {
            let mut manager_guard = state.project_manager.lock().await;
            *manager_guard = Some(manager);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Project Manager initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Project Manager: {}", e))
        }
    }
}

#[tauri::command]
async fn create_project(metadata: marketplace::ProjectMetadata, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Creating project: {}", metadata.name);
    
    let manager_guard = state.project_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.create_project(metadata).await {
            Ok(project_id) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "project_id": project_id
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to create project: {}", e))
            }
        }
    } else {
        Err("Project Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn publish_to_marketplace(project_id: String, price: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Publishing project {} to marketplace", project_id);
    
    let manager_guard = state.project_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.publish_to_marketplace(&project_id, price).await {
            Ok(listing_id) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "listing_id": listing_id
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to publish to marketplace: {}", e))
            }
        }
    } else {
        Err("Project Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn search_marketplace(query: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Searching marketplace: {}", query);
    
    let manager_guard = state.project_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.search_marketplace(&query).await {
            Ok(listings) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "listings": listings
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to search marketplace: {}", e))
            }
        }
    } else {
        Err("Project Manager not initialized".to_string())
    }
}

// Stage 3: Cloud Training Commands

#[tauri::command]
async fn initialize_cloud_training(cloud_provider: String, api_key: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Cloud Training Manager with provider: {}", cloud_provider);
    
    let manager = cloud_training::CloudTrainingManager::new(cloud_provider, api_key);
    
    match manager.initialize().await {
        Ok(_) => {
            let mut manager_guard = state.cloud_training_manager.lock().await;
            *manager_guard = Some(manager);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Cloud Training Manager initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Cloud Training Manager: {}", e))
        }
    }
}

#[tauri::command]
async fn submit_training_job(config: cloud_training::TrainingJobConfig, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Submitting training job: {}", config.model_name);
    
    let manager_guard = state.cloud_training_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.submit_training_job(config).await {
            Ok(job_id) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "job_id": job_id
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to submit training job: {}", e))
            }
        }
    } else {
        Err("Cloud Training Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn start_training_job(job_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Starting training job: {}", job_id);
    
    let manager_guard = state.cloud_training_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.start_training_job(&job_id).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Training job started successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to start training job: {}", e))
            }
        }
    } else {
        Err("Cloud Training Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_training_job_status(job_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting training job status: {}", job_id);
    
    let manager_guard = state.cloud_training_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.get_job_status(&job_id).await {
            Ok(status) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "job_status": status
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get training job status: {}", e))
            }
        }
    } else {
        Err("Cloud Training Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn list_training_jobs(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Listing training jobs");
    
    let manager_guard = state.cloud_training_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        let jobs = manager.list_jobs().await;
        Ok(serde_json::json!({
            "status": "success",
            "jobs": jobs
        }).to_string())
    } else {
        Err("Cloud Training Manager not initialized".to_string())
    }
}

// Stage 4: Execution Graph Commands

#[tauri::command]
async fn initialize_execution_graph(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Execution Graph Manager");
    
    let manager = execution_graph::ExecutionGraphManager::new();
    
    match manager.initialize().await {
        Ok(_) => {
            let mut manager_guard = state.execution_graph_manager.lock().await;
            *manager_guard = Some(manager);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Execution Graph Manager initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Execution Graph Manager: {}", e))
        }
    }
}

#[tauri::command]
async fn create_execution_graph(graph_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Creating execution graph: {}", graph_id);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.create_graph(graph_id).await {
            Ok(graph) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "graph": graph
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to create execution graph: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn add_graph_node(graph_id: String, node: execution_graph::ExecutionNode, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Adding node to graph {}: {}", graph_id, node.id);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.add_node(&graph_id, node).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Node added successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to add node: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn add_graph_edge(graph_id: String, edge: execution_graph::ExecutionEdge, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Adding edge to graph {}: {} -> {}", graph_id, edge.from, edge.to);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.add_edge(&graph_id, edge).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Edge added successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to add edge: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_graph_visualization(graph_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting graph visualization: {}", graph_id);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.get_graph_visualization(&graph_id).await {
            Ok(visualization) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "visualization": visualization
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get graph visualization: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_execution_order(graph_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting execution order for graph: {}", graph_id);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.get_execution_order(&graph_id).await {
            Ok(order) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "execution_order": order
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get execution order: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_graph_stats(graph_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting graph stats: {}", graph_id);
    
    let manager_guard = state.execution_graph_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.get_graph_stats(&graph_id).await {
            Ok(stats) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "stats": stats
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get graph stats: {}", e))
            }
        }
    } else {
        Err("Execution Graph Manager not initialized".to_string())
    }
}

// Stage 4: Autonomy Score Commands

#[tauri::command]
async fn initialize_autonomy_calculator(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Autonomy Calculator");
    
    let calculator = autonomy::AutonomyCalculator::new();
    
    match calculator.initialize().await {
        Ok(_) => {
            let mut calculator_guard = state.autonomy_calculator.lock().await;
            *calculator_guard = Some(calculator);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Autonomy Calculator initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Autonomy Calculator: {}", e))
        }
    }
}

#[tauri::command]
async fn start_autonomy_session(session_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Starting autonomy session: {}", session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.start_session(session_id).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Autonomy session started successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to start autonomy session: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn record_autonomous_action(session_id: String, action_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Recording autonomous action: {} for session {}", action_type, session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.record_autonomous_action(&session_id, &action_type).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Autonomous action recorded successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to record autonomous action: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn record_self_correction(session_id: String, correction_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Recording self-correction: {} for session {}", correction_type, session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.record_self_correction(&session_id, &correction_type).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Self-correction recorded successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to record self-correction: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn record_human_intervention(session_id: String, intervention_type: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Recording human intervention: {} for session {}", intervention_type, session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.record_human_intervention(&session_id, &intervention_type).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Human intervention recorded successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to record human intervention: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn record_task_completion(session_id: String, success: bool, lines_added: u32, lines_removed: u32, execution_time_ms: u64, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Recording task completion for session {}: success={}", session_id, success);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.record_task_completion(&session_id, success, lines_added, lines_removed, execution_time_ms).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Task completion recorded successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to record task completion: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn calculate_autonomy_score(session_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Calculating autonomy score for session: {}", session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.calculate_score(&session_id).await {
            Ok(score) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "score": score
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to calculate autonomy score: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn get_autonomy_score(session_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting autonomy score for session: {}", session_id);
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        match calculator.get_score(&session_id).await {
            Ok(score) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "score": score
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get autonomy score: {}", e))
            }
        }
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

#[tauri::command]
async fn get_autonomy_leaderboard(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting autonomy leaderboard");
    
    let calculator_guard = state.autonomy_calculator.lock().await;
    if let Some(ref calculator) = *calculator_guard {
        let leaderboard = calculator.get_leaderboard().await;
        Ok(serde_json::json!({
            "status": "success",
            "leaderboard": leaderboard
        }).to_string())
    } else {
        Err("Autonomy Calculator not initialized".to_string())
    }
}

// Stage 4: Verification Commands

#[tauri::command]
async fn initialize_verification_manager(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Verification Manager");
    
    let manager = verification::VerificationManager::new();
    
    match manager.initialize().await {
        Ok(_) => {
            let mut manager_guard = state.verification_manager.lock().await;
            *manager_guard = Some(manager);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Verification Manager initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Verification Manager: {}", e))
        }
    }
}

#[tauri::command]
async fn run_system_verification(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Running system verification");
    
    let manager_guard = state.verification_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.run_system_check().await {
            Ok(report) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "report": report
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to run system verification: {}", e))
            }
        }
    } else {
        Err("Verification Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn run_performance_verification(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Running performance verification");
    
    let manager_guard = state.verification_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.run_performance_check().await {
            Ok(report) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "report": report
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to run performance verification: {}", e))
            }
        }
    } else {
        Err("Verification Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_verification_report(report_id: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting verification report: {}", report_id);
    
    let manager_guard = state.verification_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        match manager.get_report(&report_id).await {
            Ok(report) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "report": report
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get verification report: {}", e))
            }
        }
    } else {
        Err("Verification Manager not initialized".to_string())
    }
}

#[tauri::command]
async fn get_all_verification_reports(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting all verification reports");
    
    let manager_guard = state.verification_manager.lock().await;
    if let Some(ref manager) = *manager_guard {
        let reports = manager.get_all_reports().await;
        Ok(serde_json::json!({
            "status": "success",
            "reports": reports
        }).to_string())
    } else {
        Err("Verification Manager not initialized".to_string())
    }
}

// Stage 4: Mock API Commands

#[tauri::command]
async fn initialize_mock_api(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing Mock API Service");
    
    let service = mock_api::MockAPIService::new();
    
    match service.initialize().await {
        Ok(_) => {
            let mut service_guard = state.mock_api_service.lock().await;
            *service_guard = Some(service);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "Mock API Service initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize Mock API Service: {}", e))
        }
    }
}

#[tauri::command]
async fn register_mock_endpoint(endpoint: mock_api::MockEndpoint, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Registering mock endpoint: {}", endpoint.endpoint_id);
    
    let service_guard = state.mock_api_service.lock().await;
    if let Some(ref service) = *service_guard {
        match service.register_endpoint(endpoint).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Mock endpoint registered successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to register mock endpoint: {}", e))
            }
        }
    } else {
        Err("Mock API Service not initialized".to_string())
    }
}

#[tauri::command]
async fn handle_mock_request(request: mock_api::MockRequest, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Handling mock request: {} {}", request.method, request.path);
    
    let service_guard = state.mock_api_service.lock().await;
    if let Some(ref service) = *service_guard {
        match service.handle_request(request).await {
            Ok(response) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "response": response
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to handle mock request: {}", e))
            }
        }
    } else {
        Err("Mock API Service not initialized".to_string())
    }
}

#[tauri::command]
async fn get_mock_api_stats(state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting Mock API stats");
    
    let service_guard = state.mock_api_service.lock().await;
    if let Some(ref service) = *service_guard {
        let stats = service.get_stats().await;
        Ok(serde_json::json!({
            "status": "success",
            "stats": stats
        }).to_string())
    } else {
        Err("Mock API Service not initialized".to_string())
    }
}

// Stage 4: LSP Commands

#[tauri::command]
async fn initialize_lsp_client(language: String, server_command: String, server_args: Vec<String>, workspace_root: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Initializing LSP client for language: {}", language);
    
    let config = lsp::LSPConfig {
        language: language.clone(),
        server_command,
        server_args,
        workspace_root,
        enabled: true,
    };
    
    let mut client = lsp::LSPClient::new(config);
    
    match client.initialize().await {
        Ok(_) => {
            let mut clients_guard = state.lsp_clients.lock().await;
            clients_guard.insert(language, client);
            
            Ok(serde_json::json!({
                "status": "success",
                "message": "LSP client initialized successfully"
            }).to_string())
        }
        Err(e) => {
            Err(format!("Failed to initialize LSP client: {}", e))
        }
    }
}

#[tauri::command]
async fn lsp_open_document(language: String, uri: String, language_id: String, text: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Opening document in LSP: {}", uri);
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(client) = clients_guard.get_mut(&language) {
        match client.open_document(uri, language_id, text).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Document opened successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to open document: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not initialized for language: {}", language))
    }
}

#[tauri::command]
async fn lsp_change_document(language: String, uri: String, text: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Changing document in LSP: {}", uri);
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(client) = clients_guard.get_mut(&language) {
        match client.change_document(uri, text).await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Document changed successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to change document: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not initialized for language: {}", language))
    }
}

#[tauri::command]
async fn lsp_get_completions(language: String, uri: String, line: u32, character: u32, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting completions for {} at {}:{}", uri, line, character);
    
    let position = lsp::LSPPosition { line, character };
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(client) = clients_guard.get_mut(&language) {
        match client.get_completions(uri, position).await {
            Ok(completions) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "completions": completions
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get completions: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not initialized for language: {}", language))
    }
}

#[tauri::command]
async fn lsp_get_hover(language: String, uri: String, line: u32, character: u32, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting hover for {} at {}:{}", uri, line, character);
    
    let position = lsp::LSPPosition { line, character };
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(client) = clients_guard.get_mut(&language) {
        match client.get_hover(uri, position).await {
            Ok(hover) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "hover": hover
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get hover: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not initialized for language: {}", language))
    }
}

#[tauri::command]
async fn lsp_get_definition(language: String, uri: String, line: u32, character: u32, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Getting definition for {} at {}:{}", uri, line, character);
    
    let position = lsp::LSPPosition { line, character };
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(client) = clients_guard.get_mut(&language) {
        match client.get_definition(uri, position).await {
            Ok(definitions) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "definitions": definitions
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to get definition: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not initialized for language: {}", language))
    }
}

#[tauri::command]
async fn lsp_shutdown(language: String, state: State<'_, AppState>) -> Result<String, String> {
    log::info!("Shutting down LSP client for language: {}", language);
    
    let mut clients_guard = state.lsp_clients.lock().await;
    if let Some(mut client) = clients_guard.remove(&language) {
        match client.shutdown().await {
            Ok(_) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "LSP client shut down successfully"
                }).to_string())
            }
            Err(e) => {
                Err(format!("Failed to shutdown LSP client: {}", e))
            }
        }
    } else {
        Err(format!("LSP client not found for language: {}", language))
    }
}

// ==================== Stage 5: Production Features ====================

#[tauri::command]
async fn log_message(level: String, target: String, message: String, state: State<'_, AppState>) -> Result<String, String> {
    let log_level = logging::LogLevel::from_str(&level).unwrap_or(logging::LogLevel::Info);
    let entry = logging::LogEntry::new(log_level, &target, &message);
    state.logging_manager.log(entry).await;
    Ok(serde_json::json!({"status": "logged"}).to_string())
}

#[tauri::command]
async fn query_logs(min_level: Option<String>, limit: Option<usize>, state: State<'_, AppState>) -> Result<String, String> {
    let level = min_level.and_then(|l| logging::LogLevel::from_str(&l));
    let entries = state.logging_manager.query_logs(level, limit.unwrap_or(100)).await;
    Ok(serde_json::json!({"status": "success", "logs": entries}).to_string())
}

#[tauri::command]
async fn get_monitoring_stats(state: State<'_, AppState>) -> Result<String, String> {
    let stats = state.logging_manager.get_stats().await;
    let counters = state.logging_manager.get_counters().await;
    let gauges = state.logging_manager.get_gauges().await;
    let health = state.logging_manager.run_health_checks().await;
    Ok(serde_json::json!({
        "status": "success",
        "stats": stats,
        "counters": counters,
        "gauges": gauges,
        "health": health
    }).to_string())
}

#[tauri::command]
async fn record_metric(name: String, duration_ms: f64, success: bool, state: State<'_, AppState>) -> Result<String, String> {
    let metric = performance::OperationMetric {
        name,
        duration_ms,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        success,
        metadata: HashMap::new(),
    };
    state.performance_monitor.record(metric).await;
    Ok(serde_json::json!({"status": "recorded"}).to_string())
}

#[tauri::command]
async fn get_performance_aggregates(state: State<'_, AppState>) -> Result<String, String> {
    let aggregates = state.performance_monitor.get_aggregates().await;
    let slowest = state.performance_monitor.get_slowest_operations(10).await;
    Ok(serde_json::json!({
        "status": "success",
        "aggregates": aggregates,
        "slowest": slowest
    }).to_string())
}

#[tauri::command]
async fn run_benchmark_command(name: String, iterations: u64, state: State<'_, AppState>) -> Result<String, String> {
    // Benchmark a trivial async op to validate the pipeline end-to-end
    let result = performance::run_benchmark(&name, iterations, || async {
        tokio::time::sleep(std::time::Duration::from_micros(100)).await;
        Ok(())
    }).await.map_err(|e| e.to_string())?;

    state.audit_logger.log_event(
        audit::AuditEventType::Custom("benchmark_run".to_string()),
        "system",
        &format!("Benchmark '{}' completed", name),
    ).await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"status": "success", "result": result}).to_string())
}

#[tauri::command]
async fn create_user(username: String, email: String, role: String, state: State<'_, AppState>) -> Result<String, String> {
    let parsed_role = user_mgmt::Role::from_str(&role)
        .ok_or_else(|| format!("Invalid role: {}", role))?;

    let result = state.user_manager.create_user(&username, &email, parsed_role)
        .await
        .map_err(|e| e.to_string())?;

    if result.success {
        if let Some(ref user) = result.user {
            state.audit_logger.log_event(
                audit::AuditEventType::UserCreated,
                &user.user_id,
                &format!("User '{}' created with role {}", username, role),
            ).await.map_err(|e| e.to_string())?;
        }
    }

    Ok(serde_json::to_string(&result).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn list_users(state: State<'_, AppState>) -> Result<String, String> {
    let users = state.user_manager.list_users().await;
    Ok(serde_json::json!({"status": "success", "users": users}).to_string())
}

#[tauri::command]
async fn get_user(user_id: String, state: State<'_, AppState>) -> Result<String, String> {
    match state.user_manager.get_user(&user_id).await {
        Some(user) => Ok(serde_json::json!({"status": "success", "user": user}).to_string()),
        None => Err(format!("User not found: {}", user_id)),
    }
}

#[tauri::command]
async fn set_user_role(user_id: String, role: String, state: State<'_, AppState>) -> Result<String, String> {
    let parsed_role = user_mgmt::Role::from_str(&role)
        .ok_or_else(|| format!("Invalid role: {}", role))?;
    let changed = state.user_manager.set_role(&user_id, parsed_role)
        .await
        .map_err(|e| e.to_string())?;
    if changed {
        state.audit_logger.log_event(
            audit::AuditEventType::RoleChanged,
            "system",
            &format!("User '{}' role set to {}", user_id, role),
        ).await.map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"status": "success"}).to_string())
    } else {
        Err(format!("User not found: {}", user_id))
    }
}

#[tauri::command]
async fn check_user_permission(user_id: String, permission: String, state: State<'_, AppState>) -> Result<String, String> {
    // Parse permission from snake_case name
    let perm = parse_permission(&permission)
        .ok_or_else(|| format!("Unknown permission: {}", permission))?;
    let allowed = state.user_manager.check_permission(&user_id, &perm).await;
    if !allowed {
        state.audit_logger.log_event(
            audit::AuditEventType::PermissionDenied,
            &user_id,
            &format!("Denied permission {}", permission),
        ).await.map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({"status": "success", "allowed": allowed}).to_string())
}

fn parse_permission(s: &str) -> Option<user_mgmt::Permission> {
    use user_mgmt::Permission::*;
    match s {
        "project_read" => Some(ProjectRead),
        "project_write" => Some(ProjectWrite),
        "project_delete" => Some(ProjectDelete),
        "project_publish" => Some(ProjectPublish),
        "execute_code" => Some(ExecuteCode),
        "execute_sandboxed" => Some(ExecuteSandboxed),
        "execute_privileged" => Some(ExecutePrivileged),
        "manage_users" => Some(ManageUsers),
        "manage_roles" => Some(ManageRoles),
        "view_audit_log" => Some(ViewAuditLog),
        "manage_api_keys" => Some(ManageApiKeys),
        "submit_training_job" => Some(SubmitTrainingJob),
        "manage_models" => Some(ManageModels),
        "view_metrics" => Some(ViewMetrics),
        "view_logs" => Some(ViewLogs),
        "shutdown_system" => Some(ShutdownSystem),
        _ => None,
    }
}

#[tauri::command]
async fn record_audit_event(event_type: String, actor: String, action: String, resource: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let et = parse_audit_event_type(&event_type);
    state.audit_logger.record(
        et,
        audit::AuditSeverity::Info,
        &actor,
        &action,
        resource,
        HashMap::new(),
        None,
    ).await.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"status": "recorded"}).to_string())
}


fn parse_audit_event_type(s: &str) -> audit::AuditEventType {
    use audit::AuditEventType::*;
    match s {
        "login_success" => LoginSuccess,
        "login_failure" => LoginFailure,
        "logout" => Logout,
        "token_refresh" => TokenRefresh,
        "permission_granted" => PermissionGranted,
        "permission_denied" => PermissionDenied,
        "role_changed" => RoleChanged,
        "code_executed" => CodeExecuted,
        "sandbox_violation" => SandboxViolation,
        "privileged_command" => PrivilegedCommand,
        "file_read" => FileRead,
        "file_write" => FileWrite,
        "file_delete" => FileDelete,
        "sensitive_data_access" => SensitiveDataAccess,
        "config_changed" => ConfigChanged,
        "user_created" => UserCreated,
        "user_deleted" => UserDeleted,
        "service_started" => ServiceStarted,
        "service_stopped" => ServiceStopped,
        "violation_recorded" => ViolationRecorded,
        "rate_limit_exceeded" => RateLimitExceeded,
        "suspicious_activity" => SuspiciousActivity,
        "injection_attempt" => InjectionAttempt,
        other => Custom(other.to_string()),
    }
}

#[tauri::command]
async fn query_audit_log(actor: Option<String>, limit: Option<usize>, state: State<'_, AppState>) -> Result<String, String> {
    let filter = audit::AuditFilter {
        actor,
        limit,
        ..Default::default()
    };
    let records = state.audit_logger.query(&filter).await;
    Ok(serde_json::json!({"status": "success", "records": records}).to_string())
}

#[tauri::command]
async fn verify_audit_chain(state: State<'_, AppState>) -> Result<String, String> {
    let valid = state.audit_logger.verify_chain().await;
    Ok(serde_json::json!({"status": "success", "chain_valid": valid}).to_string())
}

#[tauri::command]
async fn get_audit_stats(state: State<'_, AppState>) -> Result<String, String> {
    let mut stats = state.audit_logger.get_stats().await;
    stats.chain_valid = state.audit_logger.verify_chain().await;
    Ok(serde_json::json!({"status": "success", "stats": stats}).to_string())
}

#[tauri::command]
async fn detect_file_language(file_path: String, content: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let detection = state.language_manager
        .detect_language_with_content(&file_path, content.as_deref())
        .await;
    Ok(serde_json::to_string(&detection).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn list_languages(state: State<'_, AppState>) -> Result<String, String> {
    let languages = state.language_manager.list_enabled_languages().await;
    Ok(serde_json::json!({"status": "success", "languages": languages}).to_string())
}

#[tauri::command]
async fn translate_string(key: String, state: State<'_, AppState>) -> Result<String, String> {
    let text = state.language_manager.translate(&key).await;
    Ok(serde_json::json!({"status": "success", "key": key, "text": text}).to_string())
}

#[tauri::command]
async fn set_locale(locale: String, state: State<'_, AppState>) -> Result<String, String> {
    state.language_manager.set_locale(&locale).await;
    Ok(serde_json::json!({"status": "success", "locale": locale}).to_string())
}

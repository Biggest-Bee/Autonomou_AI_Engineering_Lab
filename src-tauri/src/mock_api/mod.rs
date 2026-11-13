// Mock API Service Module
// Implements mock API services for development and testing

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use log::{info, error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock API endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEndpoint {
    pub endpoint_id: String,
    pub path: String,
    pub method: HttpMethod,
    pub response: serde_json::Value,
    pub status_code: u16,
    pub delay_ms: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// Mock API request
impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::PATCH => write!(f, "PATCH"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRequest {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub query_params: HashMap<String, String>,
}

/// Mock API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
    pub response_time_ms: u64,
}

/// Mock API service
pub struct MockAPIService {
    endpoints: Arc<RwLock<HashMap<String, MockEndpoint>>>,
    request_log: Arc<RwLock<Vec<MockRequest>>>,
}

impl MockAPIService {
    pub fn new() -> Self {
        info!("Initializing Mock API Service");

        let mut endpoints = HashMap::new();
        
        // Register default endpoints
        endpoints.insert("health".to_string(), MockEndpoint {
            endpoint_id: "health".to_string(),
            path: "/health".to_string(),
            method: HttpMethod::GET,
            response: serde_json::json!({"status": "healthy", "timestamp": chrono::Utc::now().timestamp()}),
            status_code: 200,
            delay_ms: 0,
            enabled: true,
        });

        endpoints.insert("api_users".to_string(), MockEndpoint {
            endpoint_id: "api_users".to_string(),
            path: "/api/users".to_string(),
            method: HttpMethod::GET,
            response: serde_json::json!({
                "users": [
                    {"id": 1, "name": "John Doe", "email": "john@example.com"},
                    {"id": 2, "name": "Jane Smith", "email": "jane@example.com"}
                ]
            }),
            status_code: 200,
            delay_ms: 100,
            enabled: true,
        });

        endpoints.insert("api_projects".to_string(), MockEndpoint {
            endpoint_id: "api_projects".to_string(),
            path: "/api/projects".to_string(),
            method: HttpMethod::GET,
            response: serde_json::json!({
                "projects": [
                    {"id": "proj_1", "name": "AI Assistant", "status": "active"},
                    {"id": "proj_2", "name": "Code Analyzer", "status": "development"}
                ]
            }),
            status_code: 200,
            delay_ms: 150,
            enabled: true,
        });

        Self {
            endpoints: Arc::new(RwLock::new(endpoints)),
            request_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing Mock API Service");
        Ok(())
    }

    pub async fn register_endpoint(&self, endpoint: MockEndpoint) -> Result<()> {
        info!("Registering mock endpoint: {}", endpoint.endpoint_id);

        let mut endpoints = self.endpoints.write().await;
        endpoints.insert(endpoint.endpoint_id.clone(), endpoint);
        Ok(())
    }

    pub async fn unregister_endpoint(&self, endpoint_id: &str) -> Result<()> {
        info!("Unregistering mock endpoint: {}", endpoint_id);

        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(endpoint_id);
        Ok(())
    }

    pub async fn handle_request(&self, request: MockRequest) -> Result<MockResponse> {
        info!("Handling mock request: {} {}", request.method, request.path);

        // Log the request
        {
            let mut request_log = self.request_log.write().await;
            request_log.push(request.clone());
        }

        // Find matching endpoint
        let endpoints = self.endpoints.read().await;
        let endpoint = endpoints.values().find(|ep| {
            ep.path == request.path && 
            format!("{:?}", ep.method) == format!("{:?}", request.method) &&
            ep.enabled
        });

        if let Some(endpoint) = endpoint {
            // Simulate delay
            if endpoint.delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(endpoint.delay_ms)).await;
            }

            let start_time = std::time::Instant::now();
            
            let response = MockResponse {
                status_code: endpoint.status_code,
                headers: HashMap::from([
                    ("Content-Type".to_string(), "application/json".to_string()),
                    ("X-Mock-API".to_string(), "true".to_string()),
                ]),
                body: endpoint.response.clone(),
                response_time_ms: start_time.elapsed().as_millis() as u64,
            };

            info!("Mock request handled: {} {} -> {}", request.method, request.path, endpoint.status_code);
            Ok(response)
        } else {
            // Return 404 for unknown endpoints
            Ok(MockResponse {
                status_code: 404,
                headers: HashMap::from([("Content-Type".to_string(), "application/json".to_string())]),
                body: serde_json::json!({"error": "Endpoint not found"}),
                response_time_ms: 0,
            })
        }
    }

    pub async fn get_endpoint(&self, endpoint_id: &str) -> Option<MockEndpoint> {
        let endpoints = self.endpoints.read().await;
        endpoints.get(endpoint_id).cloned()
    }

    pub async fn list_endpoints(&self) -> Vec<MockEndpoint> {
        let endpoints = self.endpoints.read().await;
        endpoints.values().cloned().collect()
    }

    pub async fn get_request_log(&self) -> Vec<MockRequest> {
        let request_log = self.request_log.read().await;
        request_log.clone()
    }

    pub async fn clear_request_log(&self) -> Result<()> {
        let mut request_log = self.request_log.write().await;
        request_log.clear();
        Ok(())
    }

    pub async fn enable_endpoint(&self, endpoint_id: &str, enabled: bool) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(endpoint_id) {
            endpoint.enabled = enabled;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Endpoint not found: {}", endpoint_id))
        }
    }

    pub async fn get_stats(&self) -> MockAPIStats {
        let endpoints = self.endpoints.read().await;
        let request_log = self.request_log.read().await;

        MockAPIStats {
            total_endpoints: endpoints.len(),
            enabled_endpoints: endpoints.values().filter(|ep| ep.enabled).count(),
            total_requests: request_log.len(),
            endpoints: endpoints.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAPIStats {
    pub total_endpoints: usize,
    pub enabled_endpoints: usize,
    pub total_requests: usize,
    pub endpoints: Vec<String>,
}

impl Default for MockAPIService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MockAPIService {
    fn drop(&mut self) {
        info!("Mock API Service shutting down");
    }
}

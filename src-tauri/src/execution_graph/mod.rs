// Execution Graph Visualization Module
// Implements DAG visualization for autonomous code execution

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use log::{info, error, warn};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Execution graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: String,
    pub node_type: NodeType,
    pub status: NodeStatus,
    pub dependencies: Vec<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    CodeGeneration,
    Refactoring,
    Testing,
    Deployment,
    Verification,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Skipped,
}

/// Execution graph edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeType {
    DependsOn,
    DataFlow,
    ControlFlow,
}

/// Execution graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    pub graph_id: String,
    pub nodes: HashMap<String, ExecutionNode>,
    pub edges: Vec<ExecutionEdge>,
    pub created_at: i64,
    pub updated_at: i64,
    pub overall_status: GraphStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Partial,
}

/// Execution graph manager
pub struct ExecutionGraphManager {
    graphs: Arc<RwLock<HashMap<String, ExecutionGraph>>>,
    active_graphs: Arc<RwLock<HashMap<String, String>>>, // session_id -> graph_id
}

impl ExecutionGraphManager {
    pub fn new() -> Self {
        info!("Initializing Execution Graph Manager");

        Self {
            graphs: Arc::new(RwLock::new(HashMap::new())),
            active_graphs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing Execution Graph Manager");
        Ok(())
    }

    pub async fn create_graph(&self, graph_id: String) -> Result<ExecutionGraph> {
        info!("Creating execution graph: {}", graph_id);

        let timestamp = chrono::Utc::now().timestamp();
        let graph = ExecutionGraph {
            graph_id: graph_id.clone(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            created_at: timestamp,
            updated_at: timestamp,
            overall_status: GraphStatus::Pending,
        };

        let mut graphs = self.graphs.write().await;
        graphs.insert(graph_id.clone(), graph.clone());

        info!("Execution graph created: {}", graph_id);
        Ok(graph)
    }

    pub async fn add_node(&self, graph_id: &str, node: ExecutionNode) -> Result<()> {
        info!("Adding node to graph {}: {}", graph_id, node.id);

        let mut graphs = self.graphs.write().await;
        if let Some(graph) = graphs.get_mut(graph_id) {
            graph.nodes.insert(node.id.clone(), node);
            graph.updated_at = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Graph not found: {}", graph_id))
        }
    }

    pub async fn add_edge(&self, graph_id: &str, edge: ExecutionEdge) -> Result<()> {
        info!("Adding edge to graph {}: {} -> {}", graph_id, edge.from, edge.to);

        let mut graphs = self.graphs.write().await;
        if let Some(graph) = graphs.get_mut(graph_id) {
            graph.edges.push(edge);
            graph.updated_at = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Graph not found: {}", graph_id))
        }
    }

    pub async fn update_node_status(&self, graph_id: &str, node_id: &str, status: NodeStatus) -> Result<()> {
        info!("Updating node status: {} -> {:?}", node_id, status);

        let mut graphs = self.graphs.write().await;
        if let Some(graph) = graphs.get_mut(graph_id) {
            if let Some(node) = graph.nodes.get_mut(node_id) {
                node.status = status;
                graph.updated_at = chrono::Utc::now().timestamp();
                Ok(())
            } else {
                Err(anyhow::anyhow!("Node not found: {}", node_id))
            }
        } else {
            Err(anyhow::anyhow!("Graph not found: {}", graph_id))
        }
    }

    pub async fn get_graph(&self, graph_id: &str) -> Result<ExecutionGraph> {
        let graphs = self.graphs.read().await;
        if let Some(graph) = graphs.get(graph_id) {
            Ok(graph.clone())
        } else {
            Err(anyhow::anyhow!("Graph not found: {}", graph_id))
        }
    }

    pub async fn get_graph_visualization(&self, graph_id: &str) -> Result<serde_json::Value> {
        let graph = self.get_graph(graph_id).await?;

        // Convert to visualization format
        let visualization = serde_json::json!({
            "nodes": graph.nodes.values().map(|node| {
                serde_json::json!({
                    "id": node.id,
                    "label": format!("{:?}", node.node_type),
                    "status": format!("{:?}", node.status),
                    "x": (node.id.parse::<u64>().unwrap_or(0) % 10) * 100,
                    "y": (node.id.parse::<u64>().unwrap_or(0) / 10) * 100
                })
            }).collect::<Vec<_>>(),
            "edges": graph.edges.iter().map(|edge| {
                serde_json::json!({
                    "source": edge.from,
                    "target": edge.to,
                    "type": format!("{:?}", edge.edge_type)
                })
            }).collect::<Vec<_>>(),
            "status": format!("{:?}", graph.overall_status)
        });

        Ok(visualization)
    }

    pub async fn get_execution_order(&self, graph_id: &str) -> Result<Vec<String>> {
        let graph = self.get_graph(graph_id).await?;
        
        // Topological sort to get execution order
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        
        // Initialize in-degree and adjacency list
        for node_id in graph.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
            adjacency.insert(node_id.clone(), Vec::new());
        }
        
        // Build adjacency list and in-degree
        for edge in &graph.edges {
            if let (Some(to_degree), Some(to_adj)) = (in_degree.get_mut(&edge.to), adjacency.get_mut(&edge.from)) {
                *to_degree += 1;
                to_adj.push(edge.to.clone());
            }
        }
        
        // Kahn's algorithm for topological sort
        let mut queue = VecDeque::new();
        let mut result = Vec::new();
        
        // Find nodes with no dependencies
        for (node_id, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(node_id.clone());
            }
        }
        
        // Process nodes
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.clone());
            
            if let Some(neighbors) = adjacency.get(&node_id) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }
        
        Ok(result)
    }

    pub async fn get_graph_stats(&self, graph_id: &str) -> Result<GraphStats> {
        let graph = self.get_graph(graph_id).await?;
        
        let total_nodes = graph.nodes.len();
        let completed_nodes = graph.nodes.values().filter(|n| matches!(n.status, NodeStatus::Completed)).count();
        let failed_nodes = graph.nodes.values().filter(|n| matches!(n.status, NodeStatus::Failed(_))).count();
        let running_nodes = graph.nodes.values().filter(|n| matches!(n.status, NodeStatus::Running)).count();
        
        let total_duration_ms = graph.nodes.values()
            .filter_map(|n| n.duration_ms)
            .sum::<u64>();
        
        Ok(GraphStats {
            total_nodes,
            completed_nodes,
            failed_nodes,
            running_nodes,
            total_edges: graph.edges.len(),
            total_duration_ms,
            overall_status: graph.overall_status.clone(),
        })
    }

    pub async fn set_active_graph(&self, session_id: String, graph_id: String) -> Result<()> {
        let mut active_graphs = self.active_graphs.write().await;
        active_graphs.insert(session_id, graph_id);
        Ok(())
    }

    pub async fn get_active_graph(&self, session_id: &str) -> Option<String> {
        let active_graphs = self.active_graphs.read().await;
        active_graphs.get(session_id).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub completed_nodes: usize,
    pub failed_nodes: usize,
    pub running_nodes: usize,
    pub total_edges: usize,
    pub total_duration_ms: u64,
    pub overall_status: GraphStatus,
}

impl Default for ExecutionGraphManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExecutionGraphManager {
    fn drop(&mut self) {
        info!("Execution Graph Manager shutting down");
    }
}

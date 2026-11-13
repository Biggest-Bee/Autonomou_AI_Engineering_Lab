// Autonomy Score Calculation Module
// Implements autonomy scoring for AI agent performance

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use log::{info, error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Autonomy score components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyScore {
    pub total_score: f32,
    pub autonomous_actions: f32,
    pub self_corrections: f32,
    pub human_interventions: f32,
    pub code_quality: f32,
    pub task_completion: f32,
    pub efficiency: f32,
    pub calculated_at: i64,
    pub session_id: String,
}

/// Autonomy metrics tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyMetrics {
    pub session_id: String,
    pub autonomous_actions: u32,
    pub self_corrections: u32,
    pub human_interventions: u32,
    pub total_tasks: u32,
    pub completed_tasks: u32,
    pub failed_tasks: u32,
    pub total_lines_added: u32,
    pub total_lines_removed: u32,
    pub total_execution_time_ms: u64,
    pub started_at: i64,
}

/// Autonomy score calculator
pub struct AutonomyCalculator {
    metrics: Arc<RwLock<HashMap<String, AutonomyMetrics>>>,
    scores: Arc<RwLock<HashMap<String, AutonomyScore>>>,
}

impl AutonomyCalculator {
    pub fn new() -> Self {
        info!("Initializing Autonomy Calculator");

        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            scores: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing Autonomy Calculator");
        Ok(())
    }

    pub async fn start_session(&self, session_id: String) -> Result<()> {
        info!("Starting autonomy session: {}", session_id);

        let metrics = AutonomyMetrics {
            session_id: session_id.clone(),
            autonomous_actions: 0,
            self_corrections: 0,
            human_interventions: 0,
            total_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            total_lines_added: 0,
            total_lines_removed: 0,
            total_execution_time_ms: 0,
            started_at: chrono::Utc::now().timestamp(),
        };

        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(session_id, metrics);

        Ok(())
    }

    pub async fn record_autonomous_action(&self, session_id: &str, action_type: &str) -> Result<()> {
        info!("Recording autonomous action: {} for session {}", action_type, session_id);

        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(session_id) {
            metrics.autonomous_actions += 1;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn record_self_correction(&self, session_id: &str, correction_type: &str) -> Result<()> {
        info!("Recording self-correction: {} for session {}", correction_type, session_id);

        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(session_id) {
            metrics.self_corrections += 1;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn record_human_intervention(&self, session_id: &str, intervention_type: &str) -> Result<()> {
        info!("Recording human intervention: {} for session {}", intervention_type, session_id);

        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(session_id) {
            metrics.human_interventions += 1;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn record_task_completion(&self, session_id: &str, success: bool, lines_added: u32, lines_removed: u32, execution_time_ms: u64) -> Result<()> {
        info!("Recording task completion for session {}: success={}", session_id, success);

        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(session_id) {
            metrics.total_tasks += 1;
            if success {
                metrics.completed_tasks += 1;
            } else {
                metrics.failed_tasks += 1;
            }
            metrics.total_lines_added += lines_added;
            metrics.total_lines_removed += lines_removed;
            metrics.total_execution_time_ms += execution_time_ms;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn calculate_score(&self, session_id: &str) -> Result<AutonomyScore> {
        info!("Calculating autonomy score for session: {}", session_id);

        let metrics_map = self.metrics.read().await;
        if let Some(metrics) = metrics_map.get(session_id) {
            // Calculate individual score components
            let autonomous_actions_score = calculate_autonomous_actions_score(metrics);
            let self_corrections_score = calculate_self_corrections_score(metrics);
            let human_interventions_score = calculate_human_interventions_score(metrics);
            let code_quality_score = calculate_code_quality_score(metrics);
            let task_completion_score = calculate_task_completion_score(metrics);
            let efficiency_score = calculate_efficiency_score(metrics);

            // Weighted total score
            let total_score = (
                autonomous_actions_score * 0.25 +
                self_corrections_score * 0.20 +
                human_interventions_score * 0.15 +
                code_quality_score * 0.15 +
                task_completion_score * 0.15 +
                efficiency_score * 0.10
            ).min(1.0);

            let score = AutonomyScore {
                total_score,
                autonomous_actions: autonomous_actions_score,
                self_corrections: self_corrections_score,
                human_interventions: human_interventions_score,
                code_quality: code_quality_score,
                task_completion: task_completion_score,
                efficiency: efficiency_score,
                calculated_at: chrono::Utc::now().timestamp(),
                session_id: session_id.to_string(),
            };

            // Store the score
            let mut scores_map = self.scores.write().await;
            scores_map.insert(session_id.to_string(), score.clone());

            info!("Autonomy score calculated: {:.2} for session {}", total_score, session_id);
            Ok(score)
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn get_score(&self, session_id: &str) -> Result<AutonomyScore> {
        let scores_map = self.scores.read().await;
        if let Some(score) = scores_map.get(session_id) {
            Ok(score.clone())
        } else {
            Err(anyhow::anyhow!("Score not found for session: {}", session_id))
        }
    }

    pub async fn get_metrics(&self, session_id: &str) -> Result<AutonomyMetrics> {
        let metrics_map = self.metrics.read().await;
        if let Some(metrics) = metrics_map.get(session_id) {
            Ok(metrics.clone())
        } else {
            Err(anyhow::anyhow!("Metrics not found for session: {}", session_id))
        }
    }

    pub async fn get_all_scores(&self) -> Vec<AutonomyScore> {
        let scores_map = self.scores.read().await;
        scores_map.values().cloned().collect()
    }

    pub async fn get_leaderboard(&self) -> Vec<AutonomyScore> {
        let mut scores: Vec<AutonomyScore> = self.get_all_scores().await;
        scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

// Score calculation functions
fn calculate_autonomous_actions_score(metrics: &AutonomyMetrics) -> f32 {
    // More autonomous actions = higher score, but with diminishing returns
    let base_score = (metrics.autonomous_actions as f32 / 100.0).min(1.0);
    let correction_bonus = (metrics.self_corrections as f32 / 10.0).min(0.2);
    (base_score + correction_bonus).min(1.0)
}

fn calculate_self_corrections_score(metrics: &AutonomyMetrics) -> f32 {
    // Self-corrections indicate good error handling
    let correction_ratio = if metrics.total_tasks > 0 {
        metrics.self_corrections as f32 / metrics.total_tasks as f32
    } else {
        0.0
    };
    (correction_ratio * 2.0).min(1.0)
}

fn calculate_human_interventions_score(metrics: &AutonomyMetrics) -> f32 {
    // Fewer human interventions = higher score
    let intervention_ratio = if metrics.total_tasks > 0 {
        metrics.human_interventions as f32 / metrics.total_tasks as f32
    } else {
        0.0
    };
    (1.0 - intervention_ratio).max(0.0)
}

fn calculate_code_quality_score(metrics: &AutonomyMetrics) -> f32 {
    // Based on lines changed vs tasks completed
    let avg_lines_per_task = if metrics.completed_tasks > 0 {
        (metrics.total_lines_added + metrics.total_lines_removed) as f32 / metrics.completed_tasks as f32
    } else {
        0.0
    };
    
    // Optimal is around 50-100 lines per task
    let quality_score = if avg_lines_per_task > 0.0 {
        let deviation = (avg_lines_per_task - 75.0).abs() / 75.0;
        (1.0 - deviation).max(0.0)
    } else {
        0.0
    };
    
    quality_score
}

fn calculate_task_completion_score(metrics: &AutonomyMetrics) -> f32 {
    // Completion rate
    if metrics.total_tasks == 0 {
        return 0.0;
    }
    
    let completion_rate = metrics.completed_tasks as f32 / metrics.total_tasks as f32;
    completion_rate
}

fn calculate_efficiency_score(metrics: &AutonomyMetrics) -> f32 {
    // Based on execution time per task
    if metrics.completed_tasks == 0 || metrics.total_execution_time_ms == 0 {
        return 0.0;
    }
    
    let avg_time_per_task = metrics.total_execution_time_ms as f32 / metrics.completed_tasks as f32;
    
    // Optimal is around 5000ms (5 seconds) per task
    let efficiency_score = if avg_time_per_task > 0.0 {
        let deviation = (avg_time_per_task - 5000.0).abs() / 5000.0;
        (1.0 - deviation).max(0.0)
    } else {
        0.0
    };
    
    efficiency_score
}

impl Default for AutonomyCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AutonomyCalculator {
    fn drop(&mut self) {
        info!("Autonomy Calculator shutting down");
    }
}

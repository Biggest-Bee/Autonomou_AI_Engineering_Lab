"""
Logging and Monitoring Module
Provides training metrics, loss tracking, and monitoring utilities
"""

import time
import json
from typing import Dict, Any, List, Optional
from dataclasses import dataclass, field
from pathlib import Path
from collections import defaultdict
import structlog

log = structlog.get_logger()


@dataclass
class TrainingMetrics:
    """Training metrics for a single step"""
    step: int
    epoch: int
    loss: float
    learning_rate: float
    gradient_norm: float = 0.0
    throughput: float = 0.0  # samples per second
    epoch_time: float = 0.0
    additional_metrics: Dict[str, float] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "step": self.step,
            "epoch": self.epoch,
            "loss": self.loss,
            "learning_rate": self.learning_rate,
            "gradient_norm": self.gradient_norm,
            "throughput": self.throughput,
            "epoch_time": self.epoch_time,
            **self.additional_metrics
        }


@dataclass
class EvaluationMetrics:
    """Evaluation metrics"""
    step: int
    epoch: int
    loss: float
    perplexity: float = 0.0
    accuracy: float = 0.0
    bleu_score: float = 0.0
    additional_metrics: Dict[str, float] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "step": self.step,
            "epoch": self.epoch,
            "loss": self.loss,
            "perplexity": self.perplexity,
            "accuracy": self.accuracy,
            "bleu_score": self.bleu_score,
            **self.additional_metrics
        }


class MetricsTracker:
    """Track and manage training metrics"""
    
    def __init__(self, log_dir: str = "./logs"):
        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        
        self.training_metrics: List[TrainingMetrics] = []
        self.evaluation_metrics: List[EvaluationMetrics] = []
        
        self.current_step = 0
        self.current_epoch = 0
        self.step_start_time = None
        self.epoch_start_time = None
        
        self.loss_history = []
        self.learning_rate_history = []
        self.gradient_norm_history = []
        
        self.best_loss = float('inf')
        self.best_step = 0
        
        log.info("metrics_tracker_initialized", log_dir=str(self.log_dir))
    
    def start_step(self):
        """Start timing a training step"""
        self.step_start_time = time.time()
    
    def end_step(self):
        """End timing a training step"""
        if self.step_start_time:
            step_time = time.time() - self.step_start_time
            return step_time
        return 0.0
    
    def start_epoch(self):
        """Start timing an epoch"""
        self.epoch_start_time = time.time()
        self.current_epoch += 1
    
    def end_epoch(self):
        """End timing an epoch"""
        if self.epoch_start_time:
            epoch_time = time.time() - self.epoch_start_time
            return epoch_time
        return 0.0
    
    def log_training_step(
        self,
        loss: float,
        learning_rate: float,
        gradient_norm: float = 0.0,
        batch_size: int = 1,
        additional_metrics: Optional[Dict[str, float]] = None
    ):
        """Log a training step"""
        step_time = self.end_step()
        throughput = batch_size / step_time if step_time > 0 else 0.0
        
        metrics = TrainingMetrics(
            step=self.current_step,
            epoch=self.current_epoch,
            loss=loss,
            learning_rate=learning_rate,
            gradient_norm=gradient_norm,
            throughput=throughput,
            epoch_time=step_time,
            additional_metrics=additional_metrics or {}
        )
        
        self.training_metrics.append(metrics)
        self.loss_history.append(loss)
        self.learning_rate_history.append(learning_rate)
        self.gradient_norm_history.append(gradient_norm)
        
        # Track best loss
        if loss < self.best_loss:
            self.best_loss = loss
            self.best_step = self.current_step
        
        self.current_step += 1
        
        log.info(
            "training_step",
            step=self.current_step,
            loss=loss,
            learning_rate=learning_rate,
            gradient_norm=gradient_norm,
            throughput=throughput
        )
    
    def log_evaluation(
        self,
        loss: float,
        perplexity: float = 0.0,
        accuracy: float = 0.0,
        bleu_score: float = 0.0,
        additional_metrics: Optional[Dict[str, float]] = None
    ):
        """Log evaluation metrics"""
        metrics = EvaluationMetrics(
            step=self.current_step,
            epoch=self.current_epoch,
            loss=loss,
            perplexity=perplexity,
            accuracy=accuracy,
            bleu_score=bleu_score,
            additional_metrics=additional_metrics or {}
        )
        
        self.evaluation_metrics.append(metrics)
        
        log.info(
            "evaluation",
            step=self.current_step,
            loss=loss,
            perplexity=perplexity,
            accuracy=accuracy
        )
    
    def get_loss_statistics(self, window: int = 100) -> Dict[str, float]:
        """Get loss statistics over a window"""
        if not self.loss_history:
            return {"mean": 0.0, "std": 0.0, "min": 0.0, "max": 0.0}
        
        recent_losses = self.loss_history[-window:]
        return {
            "mean": sum(recent_losses) / len(recent_losses),
            "std": (sum((x - sum(recent_losses) / len(recent_losses)) ** 2 for x in recent_losses) / len(recent_losses)) ** 0.5,
            "min": min(recent_losses),
            "max": max(recent_losses)
        }
    
    def get_gradient_norm_statistics(self, window: int = 100) -> Dict[str, float]:
        """Get gradient norm statistics over a window"""
        if not self.gradient_norm_history:
            return {"mean": 0.0, "max": 0.0}
        
        recent_grads = self.gradient_norm_history[-window:]
        return {
            "mean": sum(recent_grads) / len(recent_grads),
            "max": max(recent_grads)
        }
    
    def save_metrics(self, filename: str = "metrics.json"):
        """Save all metrics to file"""
        metrics_data = {
            "training": [m.to_dict() for m in self.training_metrics],
            "evaluation": [m.to_dict() for m in self.evaluation_metrics],
            "best_loss": self.best_loss,
            "best_step": self.best_step,
            "total_steps": self.current_step,
            "total_epochs": self.current_epoch
        }
        
        filepath = self.log_dir / filename
        with open(filepath, 'w') as f:
            json.dump(metrics_data, f, indent=2)
        
        log.info("metrics_saved", filepath=str(filepath))
    
    def load_metrics(self, filename: str = "metrics.json"):
        """Load metrics from file"""
        filepath = self.log_dir / filename
        if not filepath.exists():
            log.warning("metrics_file_not_found", filepath=str(filepath))
            return
        
        with open(filepath, 'r') as f:
            metrics_data = json.load(f)
        
        self.training_metrics = [
            TrainingMetrics(**m) for m in metrics_data["training"]
        ]
        self.evaluation_metrics = [
            EvaluationMetrics(**m) for m in metrics_data["evaluation"]
        ]
        self.best_loss = metrics_data["best_loss"]
        self.best_step = metrics_data["best_step"]
        self.current_step = metrics_data["total_steps"]
        self.current_epoch = metrics_data["total_epochs"]
        
        # Rebuild histories
        self.loss_history = [m.loss for m in self.training_metrics]
        self.learning_rate_history = [m.learning_rate for m in self.training_metrics]
        self.gradient_norm_history = [m.gradient_norm for m in self.training_metrics]
        
        log.info("metrics_loaded", filepath=str(filepath))
    
    def get_summary(self) -> Dict[str, Any]:
        """Get training summary"""
        loss_stats = self.get_loss_statistics()
        grad_stats = self.get_gradient_norm_statistics()
        
        return {
            "total_steps": self.current_step,
            "total_epochs": self.current_epoch,
            "best_loss": self.best_loss,
            "best_step": self.best_step,
            "current_loss": self.loss_history[-1] if self.loss_history else 0.0,
            "loss_statistics": loss_stats,
            "gradient_norm_statistics": grad_stats,
            "total_evaluations": len(self.evaluation_metrics)
        }


class ProgressLogger:
    """Simple progress logger for training"""
    
    def __init__(self, total_steps: int, log_interval: int = 10):
        self.total_steps = total_steps
        self.log_interval = log_interval
        self.current_step = 0
        self.start_time = time.time()
    
    def update(self, step: int, loss: float, additional_info: Optional[Dict[str, Any]] = None):
        """Update progress"""
        self.current_step = step
        progress = step / self.total_steps
        
        if step % self.log_interval == 0 or step == self.total_steps:
            elapsed_time = time.time() - self.start_time
            eta = (elapsed_time / step) * (self.total_steps - step) if step > 0 else 0
            
            log.info(
                "training_progress",
                step=step,
                total_steps=self.total_steps,
                progress=f"{progress:.2%}",
                loss=loss,
                elapsed_time=f"{elapsed_time:.2f}s",
                eta=f"{eta:.2f}s",
                additional_info=additional_info or {}
            )
    
    def finish(self):
        """Mark training as finished"""
        elapsed_time = time.time() - self.start_time
        log.info("training_completed", total_time=f"{elapsed_time:.2f}s")


class EarlyStopping:
    """Early stopping based on loss improvement"""
    
    def __init__(self, patience: int = 5, min_delta: float = 0.001):
        self.patience = patience
        self.min_delta = min_delta
        self.best_loss = float('inf')
        self.counter = 0
        self.should_stop = False
    
    def check(self, loss: float) -> bool:
        """Check if training should stop"""
        if loss < self.best_loss - self.min_delta:
            self.best_loss = loss
            self.counter = 0
        else:
            self.counter += 1
        
        if self.counter >= self.patience:
            self.should_stop = True
            log.info("early_stopping_triggered", best_loss=self.best_loss, patience=self.patience)
        
        return self.should_stop
    
    def reset(self):
        """Reset early stopping"""
        self.best_loss = float('inf')
        self.counter = 0
        self.should_stop = False


log.info("logging_monitoring_loaded", message="Logging and monitoring module loaded successfully")
"""
Configuration Management Module
Handles hyperparameters, training settings, and model configurations
"""

import json
import os
from typing import Dict, Any, Optional
from dataclasses import dataclass, asdict, field
from pathlib import Path
import structlog

log = structlog.get_logger()


@dataclass
class ModelConfig:
    """Model architecture configuration"""
    hidden_size: int = 768
    num_layers: int = 12
    num_heads: int = 12
    intermediate_size: int = 3072
    max_seq_length: int = 512
    vocab_size: int = 50000
    dropout: float = 0.1
    layer_norm_eps: float = 1e-5
    activation: str = "gelu"  # relu, gelu, tanh, sigmoid
    use_bias: bool = True
    embedding_dim: int = 768
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)
    
    @classmethod
    def from_dict(cls, config_dict: Dict[str, Any]) -> 'ModelConfig':
        return cls(**config_dict)
    
    def save(self, path: str):
        """Save configuration to file"""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
        log.info("model_config_saved", path=path)
    
    @classmethod
    def load(cls, path: str) -> 'ModelConfig':
        """Load configuration from file"""
        with open(path, 'r') as f:
            config_dict = json.load(f)
        return cls.from_dict(config_dict)


@dataclass
class TrainingConfig:
    """Training configuration"""
    batch_size: int = 32
    learning_rate: float = 1e-4
    num_epochs: int = 10
    warmup_steps: int = 10000
    weight_decay: float = 0.01
    gradient_clip: float = 1.0
    max_grad_norm: float = 1.0
    save_steps: int = 1000
    eval_steps: int = 500
    log_steps: int = 100
    optimizer: str = "adam"  # adam, sgd, adamw
    scheduler: str = "linear"  # linear, cosine, constant
    learning_rate_min: float = 0.0
    mixed_precision: bool = False
    gradient_accumulation_steps: int = 1
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)
    
    @classmethod
    def from_dict(cls, config_dict: Dict[str, Any]) -> 'TrainingConfig':
        return cls(**config_dict)
    
    def save(self, path: str):
        """Save configuration to file"""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
        log.info("training_config_saved", path=path)
    
    @classmethod
    def load(cls, path: str) -> 'TrainingConfig':
        """Load configuration from file"""
        with open(path, 'r') as f:
            config_dict = json.load(f)
        return cls.from_dict(config_dict)


@dataclass
class DataConfig:
    """Data processing configuration"""
    data_path: str = "./data"
    train_file: str = "train.txt"
    val_file: str = "val.txt"
    test_file: str = "test.txt"
    tokenizer_path: str = "./tokenizer"
    max_length: int = 512
    vocab_size: int = 50000
    min_frequency: int = 2
    special_tokens: Dict[str, str] = field(default_factory=lambda: {
        "pad": "<pad>",
        "unk": "<unk>",
        "bos": "<bos>",
        "eos": "<eos>",
        "mask": "<mask>"
    })
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)
    
    @classmethod
    def from_dict(cls, config_dict: Dict[str, Any]) -> 'DataConfig':
        return cls(**config_dict)
    
    def save(self, path: str):
        """Save configuration to file"""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
        log.info("data_config_saved", path=path)
    
    @classmethod
    def load(cls, path: str) -> 'DataConfig':
        """Load configuration from file"""
        with open(path, 'r') as f:
            config_dict = json.load(f)
        return cls.from_dict(config_dict)


@dataclass
class SystemConfig:
    """System and infrastructure configuration"""
    device: str = "cpu"  # cpu, cuda, mps
    num_workers: int = 4
    pin_memory: bool = True
    deterministic: bool = False
    seed: int = 42
    checkpoint_dir: str = "./checkpoints"
    log_dir: str = "./logs"
    tensorboard_dir: str = "./tensorboard"
    use_amp: bool = False
    use_ddp: bool = False
    world_size: int = 1
    local_rank: int = 0
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)
    
    @classmethod
    def from_dict(cls, config_dict: Dict[str, Any]) -> 'SystemConfig':
        return cls(**config_dict)
    
    def save(self, path: str):
        """Save configuration to file"""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
        log.info("system_config_saved", path=path)
    
    @classmethod
    def load(cls, path: str) -> 'SystemConfig':
        """Load configuration from file"""
        with open(path, 'r') as f:
            config_dict = json.load(f)
        return cls.from_dict(config_dict)


class ConfigManager:
    """Central configuration manager"""
    
    def __init__(self, config_dir: str = "./config"):
        self.config_dir = Path(config_dir)
        self.config_dir.mkdir(parents=True, exist_ok=True)
        
        self.model_config: Optional[ModelConfig] = None
        self.training_config: Optional[TrainingConfig] = None
        self.data_config: Optional[DataConfig] = None
        self.system_config: Optional[SystemConfig] = None
    
    def load_model_config(self, path: Optional[str] = None) -> ModelConfig:
        """Load model configuration"""
        if path is None:
            path = self.config_dir / "model_config.json"
        self.model_config = ModelConfig.load(str(path))
        log.info("model_config_loaded", path=str(path))
        return self.model_config
    
    def load_training_config(self, path: Optional[str] = None) -> TrainingConfig:
        """Load training configuration"""
        if path is None:
            path = self.config_dir / "training_config.json"
        self.training_config = TrainingConfig.load(str(path))
        log.info("training_config_loaded", path=str(path))
        return self.training_config
    
    def load_data_config(self, path: Optional[str] = None) -> DataConfig:
        """Load data configuration"""
        if path is None:
            path = self.config_dir / "data_config.json"
        self.data_config = DataConfig.load(str(path))
        log.info("data_config_loaded", path=str(path))
        return self.data_config
    
    def load_system_config(self, path: Optional[str] = None) -> SystemConfig:
        """Load system configuration"""
        if path is None:
            path = self.config_dir / "system_config.json"
        self.system_config = SystemConfig.load(str(path))
        log.info("system_config_loaded", path=str(path))
        return self.system_config
    
    def save_all_configs(self):
        """Save all configurations"""
        if self.model_config:
            self.model_config.save(str(self.config_dir / "model_config.json"))
        if self.training_config:
            self.training_config.save(str(self.config_dir / "training_config.json"))
        if self.data_config:
            self.data_config.save(str(self.config_dir / "data_config.json"))
        if self.system_config:
            self.system_config.save(str(self.config_dir / "system_config.json"))
        log.info("all_configs_saved")
    
    def create_default_configs(self):
        """Create default configuration files"""
        self.model_config = ModelConfig()
        self.training_config = TrainingConfig()
        self.data_config = DataConfig()
        self.system_config = SystemConfig()
        self.save_all_configs()
        log.info("default_configs_created", config_dir=str(self.config_dir))
    
    def get_hyperparameter_summary(self) -> Dict[str, Any]:
        """Get summary of all hyperparameters"""
        return {
            "model": self.model_config.to_dict() if self.model_config else {},
            "training": self.training_config.to_dict() if self.training_config else {},
            "data": self.data_config.to_dict() if self.data_config else {},
            "system": self.system_config.to_dict() if self.system_config else {}
        }


def create_experiment_config(
    experiment_name: str,
    model_config: ModelConfig,
    training_config: TrainingConfig,
    data_config: DataConfig,
    config_dir: str = "./config"
) -> str:
    """Create a complete experiment configuration"""
    config_manager = ConfigManager(config_dir)
    config_manager.model_config = model_config
    config_manager.training_config = training_config
    config_manager.data_config = data_config
    config_manager.system_config = SystemConfig()
    
    # Create experiment-specific directory
    experiment_dir = Path(config_dir) / experiment_name
    experiment_dir.mkdir(parents=True, exist_ok=True)
    
    # Save configs
    model_config.save(str(experiment_dir / "model_config.json"))
    training_config.save(str(experiment_dir / "training_config.json"))
    data_config.save(str(experiment_dir / "data_config.json"))
    config_manager.system_config.save(str(experiment_dir / "system_config.json"))
    
    log.info("experiment_config_created", experiment_name=experiment_name, path=str(experiment_dir))
    return str(experiment_dir)


log.info("config_management_loaded", message="Configuration management module loaded successfully")
"""
Custom AI Package
Foundational components for building custom AI models
"""

from .tensor_ops import (
    Tensor, zeros, ones, randn, rand, eye,
    matmul, transpose, concat, stack, split,
    squeeze, unsqueeze, clip, abs, sqrt, exp, log,
    normalize, softmax
)
from .neural_components import (
    Activation, ReLU, Sigmoid, Tanh, GELU, LeakyReLU,
    Linear, Dropout, LayerNorm, Sequential, Embedding
)
from .config import (
    ModelConfig, TrainingConfig, DataConfig, SystemConfig,
    ConfigManager, create_experiment_config
)
from .logging import (
    TrainingMetrics, EvaluationMetrics, MetricsTracker,
    ProgressLogger, EarlyStopping
)
from .preprocessing import (
    TextCleaner, BasicTokenizer, CharacterTokenizer, SubwordTokenizer,
    truncate_text, pad_sequences
)
from .file_io import (
    FileManager, CheckpointManager, ModelSaver,
    DataSaver, BackupManager
)

__version__ = "0.1.0"
__all__ = [
    # Tensor operations
    "Tensor", "zeros", "ones", "randn", "rand", "eye",
    "matmul", "transpose", "concat", "stack", "split",
    "squeeze", "unsqueeze", "clip", "abs", "sqrt", "exp", "log",
    "normalize", "softmax",
    
    # Neural components
    "Activation", "ReLU", "Sigmoid", "Tanh", "GELU", "LeakyReLU",
    "Linear", "Dropout", "LayerNorm", "Sequential", "Embedding",
    
    # Configuration
    "ModelConfig", "TrainingConfig", "DataConfig", "SystemConfig",
    "ConfigManager", "create_experiment_config",
    
    # Logging
    "TrainingMetrics", "EvaluationMetrics", "MetricsTracker",
    "ProgressLogger", "EarlyStopping",
    
    # Preprocessing
    "TextCleaner", "BasicTokenizer", "CharacterTokenizer", "SubwordTokenizer",
    "truncate_text", "pad_sequences",
    
    # File I/O
    "FileManager", "CheckpointManager", "ModelSaver",
    "DataSaver", "BackupManager"
]
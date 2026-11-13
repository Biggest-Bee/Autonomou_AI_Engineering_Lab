"""
Neural Network Components Module
Provides basic neural network building blocks
"""

import numpy as np
from typing import Tuple, Optional
from .tensor_ops import Tensor, zeros, randn, matmul
import structlog

log = structlog.get_logger()


class Activation:
    """Base activation function class"""
    
    def __call__(self, x: Tensor) -> Tensor:
        raise NotImplementedError
    
    def derivative(self, x: Tensor) -> Tensor:
        raise NotImplementedError


class ReLU(Activation):
    """Rectified Linear Unit activation"""
    
    def __call__(self, x: Tensor) -> Tensor:
        data = np.maximum(0, x.data)
        return Tensor(data, x.dtype)
    
    def derivative(self, x: Tensor) -> Tensor:
        data = (x.data > 0).astype(x.dtype)
        return Tensor(data, x.dtype)


class Sigmoid(Activation):
    """Sigmoid activation function"""
    
    def __call__(self, x: Tensor) -> Tensor:
        data = 1 / (1 + np.exp(-x.data))
        return Tensor(data, x.dtype)
    
    def derivative(self, x: Tensor) -> Tensor:
        sig = 1 / (1 + np.exp(-x.data))
        data = sig * (1 - sig)
        return Tensor(data, x.dtype)


class Tanh(Activation):
    """Hyperbolic tangent activation"""
    
    def __call__(self, x: Tensor) -> Tensor:
        data = np.tanh(x.data)
        return Tensor(data, x.dtype)
    
    def derivative(self, x: Tensor) -> Tensor:
        data = 1 - np.tanh(x.data) ** 2
        return Tensor(data, x.dtype)


class GELU(Activation):
    """Gaussian Error Linear Unit activation"""
    
    def __call__(self, x: Tensor) -> Tensor:
        # Approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
        data = 0.5 * x.data * (1 + np.tanh(np.sqrt(2 / np.pi) * (x.data + 0.044715 * x.data ** 3)))
        return Tensor(data, x.dtype)
    
    def derivative(self, x: Tensor) -> Tensor:
        # Approximate derivative
        return Tensor(np.ones_like(x.data), x.dtype)  # Simplified


class LeakyReLU(Activation):
    """Leaky ReLU activation"""
    
    def __init__(self, negative_slope: float = 0.01):
        self.negative_slope = negative_slope
    
    def __call__(self, x: Tensor) -> Tensor:
        data = np.where(x.data > 0, x.data, self.negative_slope * x.data)
        return Tensor(data, x.dtype)
    
    def derivative(self, x: Tensor) -> Tensor:
        data = np.where(x.data > 0, 1.0, self.negative_slope)
        return Tensor(data, x.dtype)


class Linear:
    """Linear (fully connected) layer"""
    
    def __init__(self, in_features: int, out_features: int, bias: bool = True):
        self.in_features = in_features
        self.out_features = out_features
        self.bias = bias
        
        # Initialize weights (Xavier/Glorot initialization)
        scale = np.sqrt(2.0 / (in_features + out_features))
        self.weight = Tensor(randn((in_features, out_features)) * scale)
        
        if bias:
            self.bias_param = Tensor(zeros((out_features,)))
        else:
            self.bias_param = None
        
        self.input_cache = None  # Cache input for backprop
    
    def __call__(self, x: Tensor) -> Tensor:
        """Forward pass"""
        self.input_cache = x
        output = matmul(x, self.weight)
        
        if self.bias_param is not None:
            output = output + self.bias_param
        
        return output
    
    def get_parameters(self) -> list:
        """Get learnable parameters"""
        params = [self.weight]
        if self.bias_param is not None:
            params.append(self.bias_param)
        return params


class Dropout:
    """Dropout layer for regularization"""
    
    def __init__(self, p: float = 0.5):
        self.p = p  # Probability of dropping a unit
        self.mask = None
        self.training = True
    
    def __call__(self, x: Tensor) -> Tensor:
        """Forward pass"""
        if self.training:
            if self.p > 0:
                self.mask = (np.random.rand(*x.shape) > self.p).astype(x.dtype) / (1 - self.p)
                return Tensor(x.data * self.mask, x.dtype)
        return x
    
    def train(self):
        """Set to training mode"""
        self.training = True
    
    def eval(self):
        """Set to evaluation mode"""
        self.training = False


class LayerNorm:
    """Layer normalization"""
    
    def __init__(self, normalized_shape: int, eps: float = 1e-5):
        self.normalized_shape = normalized_shape
        self.eps = eps
        self.gamma = Tensor(np.ones(normalized_shape))
        self.beta = Tensor(np.zeros(normalized_shape))
        self.mean_cache = None
        self.var_cache = None
    
    def __call__(self, x: Tensor) -> Tensor:
        """Forward pass"""
        # Compute mean and variance
        mean = x.data.mean(axis=-1, keepdims=True)
        var = x.data.var(axis=-1, keepdims=True)
        
        self.mean_cache = mean
        self.var_cache = var
        
        # Normalize
        normalized = (x.data - mean) / np.sqrt(var + self.eps)
        
        # Scale and shift
        output = normalized * self.gamma.data + self.beta.data
        return Tensor(output, x.dtype)
    
    def get_parameters(self) -> list:
        """Get learnable parameters"""
        return [self.gamma, self.beta]


class Sequential:
    """Sequential container for neural network layers"""
    
    def __init__(self, *layers):
        self.layers = list(layers)
    
    def __call__(self, x: Tensor) -> Tensor:
        """Forward pass through all layers"""
        for layer in self.layers:
            x = layer(x)
        return x
    
    def train(self):
        """Set all layers to training mode"""
        for layer in self.layers:
            if hasattr(layer, 'train'):
                layer.train()
    
    def eval(self):
        """Set all layers to evaluation mode"""
        for layer in self.layers:
            if hasattr(layer, 'eval'):
                layer.eval()
    
    def get_parameters(self) -> list:
        """Get all learnable parameters"""
        params = []
        for layer in self.layers:
            if hasattr(layer, 'get_parameters'):
                params.extend(layer.get_parameters())
        return params


class Embedding:
    """Embedding layer for discrete inputs"""
    
    def __init__(self, num_embeddings: int, embedding_dim: int):
        self.num_embeddings = num_embeddings
        self.embedding_dim = embedding_dim
        
        # Initialize embeddings
        scale = np.sqrt(1.0 / embedding_dim)
        self.weight = Tensor(randn((num_embeddings, embedding_dim)) * scale)
    
    def __call__(self, indices: Tensor) -> Tensor:
        """Forward pass - lookup embeddings"""
        # Convert indices to integers
        idx = indices.data.astype(int)
        
        # Look up embeddings
        embeddings = self.weight.data[idx]
        return Tensor(embeddings, self.weight.dtype)
    
    def get_parameters(self) -> list:
        """Get learnable parameters"""
        return [self.weight]


log.info("neural_network_components_loaded", message="Neural network components module loaded successfully")
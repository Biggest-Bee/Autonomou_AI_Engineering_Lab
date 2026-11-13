"""
Basic Tensor Operations Module
Provides fundamental tensor operations for neural network building
"""

import numpy as np
from typing import Union, List, Tuple
import structlog

log = structlog.get_logger()


class Tensor:
    """
    Basic tensor class for neural network operations
    Wraps numpy arrays with additional functionality
    """
    
    def __init__(self, data: Union[np.ndarray, List, float, int], dtype: str = 'float32'):
        if isinstance(data, (list, float, int)):
            data = np.array(data, dtype=dtype)
        elif isinstance(data, np.ndarray):
            data = data.astype(dtype)
        else:
            raise TypeError(f"Unsupported data type: {type(data)}")
        
        self.data = data
        self.dtype = dtype
        self.shape = data.shape
        self.grad = None  # For backpropagation
        self.requires_grad = False
    
    def __repr__(self) -> str:
        return f"Tensor(shape={self.shape}, dtype={self.dtype})"
    
    def numpy(self) -> np.ndarray:
        """Convert to numpy array"""
        return self.data
    
    def zeros_like(self) -> 'Tensor':
        """Create tensor of zeros with same shape"""
        return Tensor(np.zeros_like(self.data), self.dtype)
    
    def ones_like(self) -> 'Tensor':
        """Create tensor of ones with same shape"""
        return Tensor(np.ones_like(self.data), self.dtype)
    
    def reshape(self, new_shape: Tuple[int, ...]) -> 'Tensor':
        """Reshape tensor"""
        return Tensor(self.data.reshape(new_shape), self.dtype)
    
    def transpose(self, axes: Tuple[int, ...] = None) -> 'Tensor':
        """Transpose tensor"""
        return Tensor(self.data.transpose(axes), self.dtype)
    
    def sum(self, axis: int = None, keepdims: bool = False) -> 'Tensor':
        """Sum tensor elements"""
        return Tensor(self.data.sum(axis=axis, keepdims=keepdims), self.dtype)
    
    def mean(self, axis: int = None, keepdims: bool = False) -> 'Tensor':
        """Mean of tensor elements"""
        return Tensor(self.data.mean(axis=axis, keepdims=keepdims), self.dtype)
    
    def max(self, axis: int = None, keepdims: bool = False) -> 'Tensor':
        """Maximum of tensor elements"""
        return Tensor(self.data.max(axis=axis, keepdims=keepdims), self.dtype)
    
    def min(self, axis: int = None, keepdims: bool = False) -> 'Tensor':
        """Minimum of tensor elements"""
        return Tensor(self.data.min(axis=axis, keepdims=keepdims), self.dtype)
    
    # Arithmetic operations
    def __add__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(self.data + other.data, self.dtype)
        return Tensor(self.data + other, self.dtype)
    
    def __radd__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        return self.__add__(other)
    
    def __sub__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(self.data - other.data, self.dtype)
        return Tensor(self.data - other, self.dtype)
    
    def __rsub__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(other.data - self.data, self.dtype)
        return Tensor(other - self.data, self.dtype)
    
    def __mul__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(self.data * other.data, self.dtype)
        return Tensor(self.data * other, self.dtype)
    
    def __rmul__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        return self.__mul__(other)
    
    def __truediv__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(self.data / other.data, self.dtype)
        return Tensor(self.data / other, self.dtype)
    
    def __rtruediv__(self, other: Union['Tensor', np.ndarray, float, int]) -> 'Tensor':
        if isinstance(other, Tensor):
            return Tensor(other.data / self.data, self.dtype)
        return Tensor(other / self.data, self.dtype)
    
    def __pow__(self, power: Union[float, int]) -> 'Tensor':
        return Tensor(self.data ** power, self.dtype)
    
    def __neg__(self) -> 'Tensor':
        return Tensor(-self.data, self.dtype)
    
    def __matmul__(self, other: 'Tensor') -> 'Tensor':
        """Matrix multiplication"""
        return Tensor(self.data @ other.data, self.dtype)


def zeros(shape: Tuple[int, ...], dtype: str = 'float32') -> Tensor:
    """Create tensor filled with zeros"""
    return Tensor(np.zeros(shape, dtype=dtype), dtype)


def ones(shape: Tuple[int, ...], dtype: str = 'float32') -> Tensor:
    """Create tensor filled with ones"""
    return Tensor(np.ones(shape, dtype=dtype), dtype)


def randn(shape: Tuple[int, ...], dtype: str = 'float32') -> Tensor:
    """Create tensor with random normal distribution"""
    return Tensor(np.random.randn(*shape).astype(dtype), dtype)


def rand(shape: Tuple[int, ...], dtype: str = 'float32') -> Tensor:
    """Create tensor with random uniform distribution [0, 1)"""
    return Tensor(np.random.rand(*shape).astype(dtype), dtype)


def eye(n: int, dtype: str = 'float32') -> Tensor:
    """Create identity matrix"""
    return Tensor(np.eye(n, dtype=dtype), dtype)


def matmul(a: Tensor, b: Tensor) -> Tensor:
    """Matrix multiplication"""
    return a @ b


def transpose(tensor: Tensor, axes: Tuple[int, ...] = None) -> Tensor:
    """Transpose tensor"""
    return tensor.transpose(axes)


def concat(tensors: List[Tensor], axis: int = 0) -> Tensor:
    """Concatenate tensors along specified axis"""
    data_list = [t.data for t in tensors]
    return Tensor(np.concatenate(data_list, axis=axis), tensors[0].dtype)


def stack(tensors: List[Tensor], axis: int = 0) -> Tensor:
    """Stack tensors along new axis"""
    data_list = [t.data for t in tensors]
    return Tensor(np.stack(data_list, axis=axis), tensors[0].dtype)


def split(tensor: Tensor, indices_or_sections: Union[int, List[int]], axis: int = 0) -> List[Tensor]:
    """Split tensor into multiple tensors"""
    split_data = np.split(tensor.data, indices_or_sections, axis=axis)
    return [Tensor(data, tensor.dtype) for data in split_data]


def squeeze(tensor: Tensor, axis: int = None) -> Tensor:
    """Remove single-dimensional entries"""
    return Tensor(np.squeeze(tensor.data, axis=axis), tensor.dtype)


def unsqueeze(tensor: Tensor, axis: int) -> Tensor:
    """Add single-dimensional entry"""
    return Tensor(np.expand_dims(tensor.data, axis=axis), tensor.dtype)


def clip(tensor: Tensor, min_val: float, max_val: float) -> Tensor:
    """Clip tensor values to range"""
    return Tensor(np.clip(tensor.data, min_val, max_val), tensor.dtype)


def abs(tensor: Tensor) -> Tensor:
    """Absolute value"""
    return Tensor(np.abs(tensor.data), tensor.dtype)


def sqrt(tensor: Tensor) -> Tensor:
    """Square root"""
    return Tensor(np.sqrt(tensor.data), tensor.dtype)


def exp(tensor: Tensor) -> Tensor:
    """Exponential"""
    return Tensor(np.exp(tensor.data), tensor.dtype)


def log(tensor: Tensor) -> Tensor:
    """Natural logarithm"""
    return Tensor(np.log(tensor.data), tensor.dtype)


def normalize(tensor: Tensor, axis: int = -1, eps: float = 1e-8) -> Tensor:
    """Normalize tensor along axis"""
    norm = np.sqrt((tensor.data ** 2).sum(axis=axis, keepdims=True) + eps)
    return Tensor(tensor.data / norm, tensor.dtype)


def softmax(tensor: Tensor, axis: int = -1) -> Tensor:
    """Softmax activation"""
    exp_data = np.exp(tensor.data - np.max(tensor.data, axis=axis, keepdims=True))
    return Tensor(exp_data / exp_data.sum(axis=axis, keepdims=True), tensor.dtype)


log.info("tensor_ops_module_loaded", message="Basic tensor operations module loaded successfully")
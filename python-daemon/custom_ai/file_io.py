"""
File I/O Utilities Module
Provides saving/loading models, checkpoints, and data management
"""

import json
import pickle
import shutil
from typing import Any, Dict, Optional
from pathlib import Path
from datetime import datetime
import structlog

log = structlog.get_logger()


class FileManager:
    """Basic file management utilities"""
    
    @staticmethod
    def ensure_dir(path: str) -> Path:
        """Ensure directory exists"""
        path_obj = Path(path)
        path_obj.mkdir(parents=True, exist_ok=True)
        return path_obj
    
    @staticmethod
    def save_json(data: Dict[str, Any], path: str) -> None:
        """Save data as JSON"""
        path_obj = Path(path)
        path_obj.parent.mkdir(parents=True, exist_ok=True)
        
        with open(path_obj, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        
        log.info("json_saved", path=str(path_obj))
    
    @staticmethod
    def load_json(path: str) -> Optional[Dict[str, Any]]:
        """Load JSON data"""
        path_obj = Path(path)
        if not path_obj.exists():
            log.warning("json_file_not_found", path=str(path_obj))
            return None
        
        with open(path_obj, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        log.info("json_loaded", path=str(path_obj))
        return data
    
    @staticmethod
    def save_pickle(data: Any, path: str) -> None:
        """Save data as pickle"""
        path_obj = Path(path)
        path_obj.parent.mkdir(parents=True, exist_ok=True)
        
        with open(path_obj, 'wb') as f:
            pickle.dump(data, f)
        
        log.info("pickle_saved", path=str(path_obj))
    
    @staticmethod
    def load_pickle(path: str) -> Optional[Any]:
        """Load pickle data"""
        path_obj = Path(path)
        if not path_obj.exists():
            log.warning("pickle_file_not_found", path=str(path_obj))
            return None
        
        with open(path_obj, 'rb') as f:
            data = pickle.load(f)
        
        log.info("pickle_loaded", path=str(path_obj))
        return data
    
    @staticmethod
    def save_text(text: str, path: str) -> None:
        """Save text to file"""
        path_obj = Path(path)
        path_obj.parent.mkdir(parents=True, exist_ok=True)
        
        with open(path_obj, 'w', encoding='utf-8') as f:
            f.write(text)
        
        log.info("text_saved", path=str(path_obj))
    
    @staticmethod
    def load_text(path: str) -> Optional[str]:
        """Load text from file"""
        path_obj = Path(path)
        if not path_obj.exists():
            log.warning("text_file_not_found", path=str(path_obj))
            return None
        
        with open(path_obj, 'r', encoding='utf-8') as f:
            text = f.read()
        
        log.info("text_loaded", path=str(path_obj))
        return text
    
    @staticmethod
    def copy_file(src: str, dst: str) -> None:
        """Copy file"""
        shutil.copy2(src, dst)
        log.info("file_copied", src=src, dst=dst)
    
    @staticmethod
    def delete_file(path: str) -> None:
        """Delete file"""
        path_obj = Path(path)
        if path_obj.exists():
            path_obj.unlink()
            log.info("file_deleted", path=str(path_obj))
    
    @staticmethod
    def delete_dir(path: str) -> None:
        """Delete directory"""
        path_obj = Path(path)
        if path_obj.exists():
            shutil.rmtree(path_obj)
            log.info("directory_deleted", path=str(path_obj))


class CheckpointManager:
    """Manage model checkpoints during training"""
    
    def __init__(self, checkpoint_dir: str = "./checkpoints", max_checkpoints: int = 5):
        self.checkpoint_dir = Path(checkpoint_dir)
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)
        self.max_checkpoints = max_checkpoints
        self.checkpoint_history: list = []
    
    def save_checkpoint(
        self,
        model_state: Dict[str, Any],
        optimizer_state: Optional[Dict[str, Any]] = None,
        step: int = 0,
        epoch: int = 0,
        loss: float = 0.0,
        additional_info: Optional[Dict[str, Any]] = None
    ) -> str:
        """Save a checkpoint"""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        checkpoint_name = f"checkpoint_step{step}_epoch{epoch}_{timestamp}"
        checkpoint_path = self.checkpoint_dir / checkpoint_name
        
        checkpoint_data = {
            "model_state": model_state,
            "optimizer_state": optimizer_state,
            "step": step,
            "epoch": epoch,
            "loss": loss,
            "timestamp": timestamp,
            "additional_info": additional_info or {}
        }
        
        # Save as pickle
        FileManager.save_pickle(checkpoint_data, str(checkpoint_path / "checkpoint.pkl"))
        
        # Save metadata as JSON
        metadata = {
            "step": step,
            "epoch": epoch,
            "loss": loss,
            "timestamp": timestamp,
            "checkpoint_name": checkpoint_name
        }
        FileManager.save_json(metadata, str(checkpoint_path / "metadata.json"))
        
        # Update history
        self.checkpoint_history.append({
            "path": str(checkpoint_path),
            "step": step,
            "epoch": epoch,
            "loss": loss,
            "timestamp": timestamp
        })
        
        # Remove old checkpoints if exceeding max
        self._cleanup_old_checkpoints()
        
        log.info("checkpoint_saved", checkpoint_name=checkpoint_name, step=step, loss=loss)
        return str(checkpoint_path)
    
    def load_checkpoint(self, checkpoint_path: str) -> Optional[Dict[str, Any]]:
        """Load a checkpoint"""
        checkpoint_path_obj = Path(checkpoint_path)
        if not checkpoint_path_obj.exists():
            log.warning("checkpoint_not_found", path=str(checkpoint_path_obj))
            return None
        
        checkpoint_data = FileManager.load_pickle(str(checkpoint_path_obj / "checkpoint.pkl"))
        if checkpoint_data:
            log.info("checkpoint_loaded", path=str(checkpoint_path_obj))
        
        return checkpoint_data
    
    def load_latest_checkpoint(self) -> Optional[Dict[str, Any]]:
        """Load the most recent checkpoint"""
        if not self.checkpoint_history:
            log.warning("no_checkpoints_available")
            return None
        
        latest = self.checkpoint_history[-1]
        return self.load_checkpoint(latest["path"])
    
    def load_best_checkpoint(self) -> Optional[Dict[str, Any]]:
        """Load the checkpoint with lowest loss"""
        if not self.checkpoint_history:
            log.warning("no_checkpoints_available")
            return None
        
        best = min(self.checkpoint_history, key=lambda x: x["loss"])
        return self.load_checkpoint(best["path"])
    
    def list_checkpoints(self) -> list:
        """List all available checkpoints"""
        return self.checkpoint_history
    
    def delete_checkpoint(self, checkpoint_path: str) -> None:
        """Delete a checkpoint"""
        FileManager.delete_dir(checkpoint_path)
        self.checkpoint_history = [
            cp for cp in self.checkpoint_history if cp["path"] != checkpoint_path
        ]
        log.info("checkpoint_deleted", path=checkpoint_path)
    
    def _cleanup_old_checkpoints(self) -> None:
        """Remove old checkpoints if exceeding max"""
        while len(self.checkpoint_history) > self.max_checkpoints:
            oldest = self.checkpoint_history.pop(0)
            self.delete_checkpoint(oldest["path"])


class ModelSaver:
    """Save and load model weights and configurations"""
    
    def __init__(self, model_dir: str = "./models"):
        self.model_dir = Path(model_dir)
        self.model_dir.mkdir(parents=True, exist_ok=True)
    
    def save_model(
        self,
        model_weights: Dict[str, Any],
        model_config: Dict[str, Any],
        model_name: str,
        additional_info: Optional[Dict[str, Any]] = None
    ) -> str:
        """Save complete model"""
        model_path = self.model_dir / model_name
        model_path.mkdir(parents=True, exist_ok=True)
        
        # Save weights
        FileManager.save_pickle(model_weights, str(model_path / "weights.pkl"))
        
        # Save config
        FileManager.save_json(model_config, str(model_path / "config.json"))
        
        # Save additional info
        if additional_info:
            FileManager.save_json(additional_info, str(model_path / "info.json"))
        
        log.info("model_saved", model_name=model_name, path=str(model_path))
        return str(model_path)
    
    def load_model(self, model_name: str) -> Optional[Dict[str, Any]]:
        """Load complete model"""
        model_path = self.model_dir / model_name
        
        if not model_path.exists():
            log.warning("model_not_found", model_name=model_name)
            return None
        
        weights = FileManager.load_pickle(str(model_path / "weights.pkl"))
        config = FileManager.load_json(str(model_path / "config.json"))
        info = FileManager.load_json(str(model_path / "info.json"))
        
        model_data = {
            "weights": weights,
            "config": config,
            "info": info
        }
        
        log.info("model_loaded", model_name=model_name)
        return model_data
    
    def load_weights_only(self, model_name: str) -> Optional[Dict[str, Any]]:
        """Load only model weights"""
        model_path = self.model_dir / model_name / "weights.pkl"
        return FileManager.load_pickle(str(model_path))
    
    def load_config_only(self, model_name: str) -> Optional[Dict[str, Any]]:
        """Load only model configuration"""
        model_path = self.model_dir / model_name / "config.json"
        return FileManager.load_json(str(model_path))
    
    def list_models(self) -> list:
        """List all saved models"""
        models = []
        for model_dir in self.model_dir.iterdir():
            if model_dir.is_dir():
                models.append({
                    "name": model_dir.name,
                    "path": str(model_dir)
                })
        return models


class DataSaver:
    """Save and load training data and datasets"""
    
    def __init__(self, data_dir: str = "./data"):
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
    
    def save_dataset(self, data: Any, dataset_name: str, format: str = "pickle") -> str:
        """Save dataset"""
        data_path = self.data_dir / f"{dataset_name}.{format}"
        
        if format == "pickle":
            FileManager.save_pickle(data, str(data_path))
        elif format == "json":
            FileManager.save_json(data, str(data_path))
        elif format == "txt":
            FileManager.save_text(data, str(data_path))
        else:
            raise ValueError(f"Unsupported format: {format}")
        
        log.info("dataset_saved", dataset_name=dataset_name, format=format)
        return str(data_path)
    
    def load_dataset(self, dataset_name: str, format: str = "pickle") -> Optional[Any]:
        """Load dataset"""
        data_path = self.data_dir / f"{dataset_name}.{format}"
        
        if not data_path.exists():
            log.warning("dataset_not_found", dataset_name=dataset_name)
            return None
        
        if format == "pickle":
            return FileManager.load_pickle(str(data_path))
        elif format == "json":
            return FileManager.load_json(str(data_path))
        elif format == "txt":
            return FileManager.load_text(str(data_path))
        else:
            raise ValueError(f"Unsupported format: {format}")
    
    def save_vocabulary(self, vocab: Dict[str, int], vocab_name: str) -> str:
        """Save vocabulary"""
        vocab_path = self.data_dir / f"{vocab_name}.json"
        FileManager.save_json(vocab, str(vocab_path))
        log.info("vocabulary_saved", vocab_name=vocab_name)
        return str(vocab_path)
    
    def load_vocabulary(self, vocab_name: str) -> Optional[Dict[str, int]]:
        """Load vocabulary"""
        vocab_path = self.data_dir / f"{vocab_name}.json"
        return FileManager.load_json(str(vocab_path))


class BackupManager:
    """Manage backups of important files"""
    
    def __init__(self, backup_dir: str = "./backups"):
        self.backup_dir = Path(backup_dir)
        self.backup_dir.mkdir(parents=True, exist_ok=True)
    
    def create_backup(self, source_path: str, backup_name: Optional[str] = None) -> str:
        """Create backup of a file or directory"""
        source = Path(source_path)
        if not source.exists():
            log.warning("source_not_found", path=str(source))
            return ""
        
        if backup_name is None:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            backup_name = f"{source.name}_{timestamp}"
        
        backup_path = self.backup_dir / backup_name
        
        if source.is_file():
            shutil.copy2(source, backup_path)
        else:
            shutil.copytree(source, backup_path)
        
        log.info("backup_created", source=str(source), backup=str(backup_path))
        return str(backup_path)
    
    def restore_backup(self, backup_path: str, destination_path: str) -> None:
        """Restore backup"""
        backup = Path(backup_path)
        destination = Path(destination_path)
        
        if not backup.exists():
            log.warning("backup_not_found", path=str(backup))
            return
        
        if backup.is_file():
            shutil.copy2(backup, destination)
        else:
            if destination.exists():
                shutil.rmtree(destination)
            shutil.copytree(backup, destination)
        
        log.info("backup_restored", backup=str(backup), destination=str(destination))
    
    def list_backups(self) -> list:
        """List all backups"""
        backups = []
        for backup in self.backup_dir.iterdir():
            backups.append({
                "name": backup.name,
                "path": str(backup),
                "is_file": backup.is_file()
            })
        return backups


log.info("file_io_utilities_loaded", message="File I/O utilities module loaded successfully")
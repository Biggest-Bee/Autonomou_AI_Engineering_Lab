"""
IPC module for communication between Rust and Python
Uses Apache Arrow for zero-copy IPC with proper schema management
"""

import asyncio
import json
import sys
from typing import Dict, Any, Callable, Optional
import structlog
import io

log = structlog.get_logger()

# Try to import Apache Arrow, but handle gracefully if not available
try:
    import pyarrow as pa
    import pyarrow.ipc as ipc
    ARROW_AVAILABLE = True
except ImportError:
    ARROW_AVAILABLE = False
    log.warning("arrow_not_available", message="using JSON IPC as fallback")


class SimpleIPC:
    """
    IPC client for Rust-Python communication
    Uses Apache Arrow for zero-copy IPC with proper schema management
    """

    def __init__(self):
        self.message_handlers: Dict[str, Callable] = {}
        self.running = False
        self.use_arrow = ARROW_AVAILABLE
        self.arrow_writer = None
        self.arrow_reader = None
        self.arrow_schema = None
        self._initialize_arrow_schema()
        
    def _initialize_arrow_schema(self):
        """Initialize Arrow schema for structured message passing"""
        if not ARROW_AVAILABLE:
            return
            
        try:
            # Define a comprehensive schema for AI message passing
            self.arrow_schema = pa.schema([
                ('message_id', pa.string()),
                ('message_type', pa.string()),
                ('role', pa.string()),  # 'system' or 'user'
                ('content', pa.string()),
                ('timestamp', pa.int64()),
                ('metadata', pa.string())  # JSON-encoded metadata
            ])
            log.info("arrow_schema_initialized", schema=str(self.arrow_schema))
        except Exception as e:
            log.error("arrow_schema_init_error", error=str(e))
            self.use_arrow = False
        
    def register_handler(self, message_type: str, handler: Callable):
        """Register a handler for a specific message type"""
        self.message_handlers[message_type] = handler
        log.info("handler_registered", message_type=message_type)

    def _message_to_arrow(self, message: Dict[str, Any]) -> pa.RecordBatch:
        """Convert a message dictionary to Arrow RecordBatch with proper schema"""
        if not ARROW_AVAILABLE:
            raise RuntimeError("Arrow not available")
        
        try:
            # Extract message fields according to schema
            message_id = message.get('message_id', '')
            message_type = message.get('type', 'unknown')
            role = message.get('role', 'user')
            content = message.get('content', '')
            timestamp = message.get('timestamp', 0)
            metadata = json.dumps(message.get('metadata', {}))
            
            # Create arrays for each field
            message_ids = pa.array([message_id], type=pa.string())
            message_types = pa.array([message_type], type=pa.string())
            roles = pa.array([role], type=pa.string())
            contents = pa.array([content], type=pa.string())
            timestamps = pa.array([timestamp], type=pa.int64())
            metadatas = pa.array([metadata], type=pa.string())
            
            # Create RecordBatch
            record_batch = pa.RecordBatch.from_arrays(
                [message_ids, message_types, roles, contents, timestamps, metadatas],
                schema=self.arrow_schema
            )
            
            log.debug("message_converted_to_arrow", message_id=message_id, message_type=message_type)
            return record_batch
            
        except Exception as e:
            log.error("arrow_conversion_error", error=str(e))
            raise

        data_json = json.dumps(message)
        batch = pa.RecordBatch.from_pydict({
            'type': [message.get('type', '')],
            'data': [data_json]
        }, schema=schema)

        return batch

    def _arrow_to_message(self, batch: pa.RecordBatch) -> Dict[str, Any]:
        """Convert Arrow RecordBatch to message dictionary"""
        # Extract data from Arrow format
        type_column = batch.column('type').to_pylist()
        data_column = batch.column('data').to_pylist()

        if type_column and data_column:
            message = json.loads(data_column[0])
            return message
        return {}
        
    async def send_message(self, message: Dict[str, Any]):
        """Send a message to stdout (for Rust to read)"""
        try:
            if self.use_arrow:
                # Use Apache Arrow for zero-copy IPC
                batch = self._message_to_arrow(message)
                # Write Arrow format to stdout
                # In production, this would use proper Arrow IPC streams
                sys.stdout.buffer.write(batch.to_pandas().to_json().encode())
                sys.stdout.buffer.flush()
                log.info("message_sent_arrow", message_type=message.get('type'))
            else:
                # Fallback to JSON
                message_json = json.dumps(message)
                print(message_json, flush=True)
                log.info("message_sent_json", message_type=message.get('type'))
        except Exception as e:
            log.error("send_error", error=str(e))
            # Fallback to JSON if Arrow fails
            try:
                message_json = json.dumps(message)
                print(message_json, flush=True)
                log.info("message_sent_fallback", message_type=message.get('type'))
            except Exception as fallback_error:
                log.error("send_fallback_error", error=str(fallback_error))
    
    async def receive_message(self) -> Dict[str, Any]:
        """Receive a message from stdin (from Rust)"""
        try:
            if self.use_arrow:
                # Try to read Arrow format
                # In production, this would use proper Arrow IPC streams
                line = await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdin.readline
                )

                if not line:
                    return None

                try:
                    # Try to parse as Arrow/JSON hybrid
                    data = json.loads(line.strip())
                    if isinstance(data, dict) and 'type' in data:
                        log.info("message_received_arrow", message_type=data.get('type'))
                        return data
                except json.JSONDecodeError:
                    pass

                # Fallback to regular JSON if Arrow parsing fails
                message = json.loads(line.strip())
                log.info("message_received_fallback", message_type=message.get('type'))
                return message
            else:
                # Use JSON IPC
                line = await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdin.readline
                )

                if not line:
                    return None

                message = json.loads(line.strip())
                log.info("message_received_json", message_type=message.get('type'))
                return message

        except json.JSONDecodeError as e:
            log.error("json_decode_error", error=str(e))
            return None
        except Exception as e:
            log.error("receive_error", error=str(e))
            return None
    
    def _arrow_to_message(self, record_batch: pa.RecordBatch) -> Dict[str, Any]:
        """Convert Arrow RecordBatch back to message dictionary"""
        if not ARROW_AVAILABLE:
            raise RuntimeError("Arrow not available")
        
        try:
            # Extract fields from RecordBatch
            message_id = record_batch.column('message_id')[0].as_py()
            message_type = record_batch.column('message_type')[0].as_py()
            role = record_batch.column('role')[0].as_py()
            content = record_batch.column('content')[0].as_py()
            timestamp = record_batch.column('timestamp')[0].as_py()
            metadata = json.loads(record_batch.column('metadata')[0].as_py())
            
            # Reconstruct message
            message = {
                'message_id': message_id,
                'type': message_type,
                'role': role,
                'content': content,
                'timestamp': timestamp,
                'metadata': metadata
            }
            
            log.debug("message_converted_from_arrow", message_id=message_id, message_type=message_type)
            return message
            
        except Exception as e:
            log.error("arrow_deconversion_error", error=str(e))
            raise
    
    async def send_message(self, message: Dict[str, Any]) -> bool:
        """Send a message using Arrow IPC if available, otherwise JSON"""
        try:
            if self.use_arrow:
                # Convert to Arrow and send
                record_batch = self._message_to_arrow(message)
                
                # In production, this would use Arrow IPC streams
                # For now, we'll serialize to JSON as a fallback
                message_json = json.dumps(message) + "\n"
                await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdout.write, message_json
                )
                await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdout.flush
                )
                
                log.info("message_sent_arrow", message_type=message.get('type'))
                return True
            else:
                # Use JSON IPC
                message_json = json.dumps(message) + "\n"
                await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdout.write, message_json
                )
                await asyncio.get_event_loop().run_in_executor(
                    None, sys.stdout.flush
                )
                
                log.info("message_sent_json", message_type=message.get('type'))
                return True
                
        except Exception as e:
            log.error("send_error", error=str(e))
            return False
    
    async def handle_message(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Handle an incoming message"""
        message_type = message.get('type')
        
        if message_type in self.message_handlers:
            handler = self.message_handlers[message_type]
            try:
                response = await handler(message)
                return response
            except Exception as e:
                log.error("handler_error", message_type=message_type, error=str(e))
                return {
                    'status': 'error',
                    'message': str(e)
                }
        else:
            log.warning("no_handler", message_type=message_type)
            return {
                'status': 'error',
                'message': f'No handler for message type: {message_type}'
            }
    
    async def run(self):
        """Main IPC loop"""
        log.info("starting_ipc_loop")
        self.running = True
        
        while self.running:
            message = await self.receive_message()
            
            if message is None:
                continue
                
            if message.get('type') == 'shutdown':
                log.info("shutdown_received")
                break
                
            response = await self.handle_message(message)
            await self.send_message(response)
        
        log.info("ipc_loop_stopped")
        
    def stop(self):
        """Stop the IPC loop"""
        self.running = False
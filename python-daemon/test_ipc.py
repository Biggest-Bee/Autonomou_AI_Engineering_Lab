"""
Tests for IPC Module
Tests Apache Arrow and JSON IPC communication
"""

import asyncio
import pytest
import json
from ipc import SimpleIPC


@pytest.fixture
def ipc():
    """Create IPC instance for testing"""
    return SimpleIPC()


@pytest.mark.asyncio
async def test_message_handlers(ipc):
    """Test message handler registration and execution"""
    # Register a test handler
    async def test_handler(message):
        return {'status': 'success', 'message': message}
    
    ipc.register_handler('test_type', test_handler)
    
    # Send a test message
    test_message = {'type': 'test_type', 'data': 'test_data'}
    response = await ipc.handle_message(test_message)
    
    # Should get a response
    assert response['status'] == 'success'
    assert response['message'] == test_message


@pytest.mark.asyncio
async def test_arrow_conversion(ipc):
    """Test Arrow message conversion"""
    if not ipc.use_arrow:
        pytest.skip("Arrow not available")
    
    # Create a test message
    test_message = {
        'message_id': 'test_123',
        'type': 'test_type',
        'role': 'user',
        'content': 'test content',
        'timestamp': 1234567890,
        'metadata': {'key': 'value'}
    }
    
    # Convert to Arrow
    record_batch = ipc._message_to_arrow(test_message)
    
    # Convert back from Arrow
    converted_message = ipc._arrow_to_message(record_batch)
    
    # Should match original message
    assert converted_message['message_id'] == test_message['message_id']
    assert converted_message['type'] == test_message['type']
    assert converted_message['role'] == test_message['role']
    assert converted_message['content'] == test_message['content']
    assert converted_message['timestamp'] == test_message['timestamp']
    assert converted_message['metadata'] == test_message['metadata']


@pytest.mark.asyncio
async def test_json_fallback(ipc):
    """Test JSON fallback when Arrow is not available"""
    # Temporarily disable Arrow
    original_use_arrow = ipc.use_arrow
    ipc.use_arrow = False
    
    try:
        # Create a test message
        test_message = {'type': 'test_type', 'data': 'test_data'}
        
        # Send message (should use JSON)
        result = await ipc.send_message(test_message)
        
        # Should succeed
        assert result is True
        
    finally:
        # Restore Arrow setting
        ipc.use_arrow = original_use_arrow


@pytest.mark.asyncio
async def test_unknown_message_type(ipc):
    """Test handling of unknown message types"""
    # Send a message with unknown type
    test_message = {'type': 'unknown_type', 'data': 'test_data'}
    
    response = await ipc.handle_message(test_message)
    
    # Should return error response
    assert response['status'] == 'error'
    assert 'No handler for message type' in response['message']


@pytest.mark.asyncio
async def test_handler_exception(ipc):
    """Test handling of exceptions in message handlers"""
    # Register a handler that raises an exception
    async def error_handler(message):
        raise ValueError("Test error")
    
    ipc.register_handler('error_type', error_handler)
    
    # Send a message that will trigger the error
    test_message = {'type': 'error_type', 'data': 'test_data'}
    
    response = await ipc.handle_message(test_message)
    
    # Should return error response
    assert response['status'] == 'error'
    assert 'Test error' in response['message']


@pytest.mark.asyncio
async def test_message_schema_validation(ipc):
    """Test message schema validation"""
    if not ipc.use_arrow:
        pytest.skip("Arrow not available")
    
    # Test message with all required fields
    valid_message = {
        'message_id': 'test_123',
        'type': 'test_type',
        'role': 'user',
        'content': 'test content',
        'timestamp': 1234567890,
        'metadata': {'key': 'value'}
    }
    
    # Should convert successfully
    record_batch = ipc._message_to_arrow(valid_message)
    converted = ipc._arrow_to_message(record_batch)
    
    # Should match original
    assert converted['message_id'] == valid_message['message_id']
    assert converted['type'] == valid_message['type']
    
    # Test message with missing fields (should use defaults)
    incomplete_message = {
        'type': 'test_type',
        'content': 'test content'
    }
    
    # Should still work with defaults
    record_batch = ipc._message_to_arrow(incomplete_message)
    converted = ipc._arrow_to_message(record_batch)
    
    # Should have default values
    assert converted['message_id'] == ''
    assert converted['role'] == 'user'
    assert converted['timestamp'] == 0


@pytest.mark.asyncio
async def test_concurrent_message_handling(ipc):
    """Test concurrent message handling"""
    # Register a handler with delay
    async def slow_handler(message):
        await asyncio.sleep(0.1)  # Simulate processing time
        return {'status': 'success', 'message': message}
    
    ipc.register_handler('slow_type', slow_handler)
    
    # Send multiple messages concurrently
    messages = [
        {'type': 'slow_type', 'data': f'test_{i}'}
        for i in range(5)
    ]
    
    # Handle all messages concurrently
    tasks = [ipc.handle_message(msg) for msg in messages]
    responses = await asyncio.gather(*tasks)
    
    # All should succeed
    for response in responses:
        assert response['status'] == 'success'


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
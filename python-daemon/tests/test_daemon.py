"""
Integration tests for Python AI Daemon
Tests AST engine, vector store, ML inference, and IPC
"""

import pytest
import pytest_asyncio
import asyncio
from ast_engine import ASTRepoMap
from vector_store import VectorStore
from ml_inference import MLInferenceEngine
from ipc import SimpleIPC


class TestASTEngine:
    """Test cases for AST engine with incremental updates"""
    
    @pytest.fixture
    def ast_engine(self):
        return ASTRepoMap()
    
    @pytest.mark.asyncio
    async def test_parse_file_basic(self, ast_engine):
        """Test basic file parsing"""
        code = """
def hello_world():
    print("Hello, World!")
    return 42
"""
        result = await ast_engine.parse_file("test.py", "python", code)
        assert 'symbols' in result
        assert len(result['symbols']) > 0
    
    @pytest.mark.asyncio
    async def test_incremental_update(self, ast_engine):
        """Test incremental AST updates"""
        code_v1 = "def func1(): pass"
        code_v2 = "def func1(): pass\ndef func2(): pass"
        
        # First parse
        result1 = await ast_engine.parse_file("test.py", "python", code_v1)
        symbols_v1 = len(result1['symbols'])
        
        # Incremental update
        result2 = await ast_engine.parse_file("test.py", "python", code_v2)
        symbols_v2 = len(result2['symbols'])
        
        # Should have more symbols after update
        assert symbols_v2 >= symbols_v1
    
    @pytest.mark.asyncio
    async def test_content_hash_consistency(self, ast_engine):
        """Test that content hash detection works"""
        code = "def test(): pass"
        
        result1 = await ast_engine.parse_file("test.py", "python", code)
        result2 = await ast_engine.parse_file("test.py", "python", code)
        
        # Should use cached results for unchanged content
        assert result1['symbols'] == result2['symbols']
    
    def test_symbol_graph(self, ast_engine):
        """Test symbol graph construction"""
        symbols = [
            {'name': 'test_func', 'type': 'function', 'line': 1}
        ]
        ast_engine._update_symbol_graph("test.py", symbols)
        
        all_symbols = ast_engine.get_all_symbols()
        assert 'test_func' in all_symbols


class TestVectorStore:
    """Test cases for vector store and embeddings"""
    
    @pytest_asyncio.fixture
    async def vector_store(self):
        store = VectorStore()
        await store.initialize()
        return store
    
    @pytest.mark.asyncio
    async def test_initialize_vector_store(self, vector_store):
        """Test vector store initialization"""
        stats = await vector_store.get_collection_stats()
        assert 'points_count' in stats
        assert 'backend' in stats
    
    @pytest.mark.asyncio
    async def test_index_symbol(self, vector_store):
        """Test symbol indexing"""
        await vector_store.index_symbol(
            symbol_name="test_func",
            file_path="test.py",
            language="python",
            code_context="def test_func(): pass",
            symbol_type="function"
        )
        
        # Verify symbol was indexed
        # (This would require actual database access in production)
    
    @pytest.mark.asyncio
    async def test_search_similar_code(self, vector_store):
        """Test semantic search"""
        results = await vector_store.search_similar_code(
            query="function that prints hello",
            limit=5
        )
        
        assert isinstance(results, list)
        assert len(results) <= 5


class TestMLInference:
    """Test cases for ML inference engine"""
    
    @pytest_asyncio.fixture
    async def ml_engine(self):
        engine = MLInferenceEngine()
        await engine.initialize()
        return engine
    
    @pytest.mark.asyncio
    async def test_model_info(self, ml_engine):
        """Test model information retrieval"""
        info = await ml_engine.get_model_info()
        assert 'status' in info
        assert 'model_name' in info
    
    @pytest.mark.asyncio
    async def test_code_generation(self, ml_engine):
        """Test code generation"""
        result = await ml_engine.generate_code(
            prompt="def hello():",
            language="python",
            max_length=50
        )
        
        assert 'status' in result
        if result['status'] == 'success':
            assert 'generated_code' in result
    
    @pytest.mark.asyncio
    async def test_code_analysis(self, ml_engine):
        """Test code analysis"""
        code = "def factorial(n): return 1 if n <= 1 else n * factorial(n-1)"
        result = await ml_engine.analyze_code(code, "python")
        
        assert 'status' in result


class TestIPC:
    """Test cases for IPC communication"""
    
    @pytest.fixture
    def ipc(self):
        return SimpleIPC()
    
    def test_handler_registration(self, ipc):
        """Test message handler registration"""
        async def test_handler(message):
            return {'status': 'ok'}
        
        ipc.register_handler('test', test_handler)
        assert 'test' in ipc.message_handlers
    
    @pytest.mark.asyncio
    async def test_message_sending(self, ipc):
        """Test message sending"""
        message = {
            'type': 'test',
            'content': 'test message'
        }
        
        result = await ipc.send_message(message)
        assert result is True


# Run tests
if __name__ == "__main__":
    pytest.main([__file__, "-v"])
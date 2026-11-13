"""
Tests for AST Engine
Tests incremental parsing, symbol extraction, and repository-wide symbol graph
"""

import asyncio
import pytest
import time
from ast_engine import ASTRepoMap


@pytest.fixture
def ast_engine():
    """Create AST engine instance for testing"""
    return ASTRepoMap()


@pytest.mark.asyncio
async def test_basic_symbol_extraction(ast_engine):
    """Test basic symbol extraction from Python code"""
    code = '''
def hello_world():
    print("Hello, World!")

class MyClass:
    def __init__(self):
        self.name = "test"
    
    def my_method(self):
        return self.name
'''
    
    result = await ast_engine.parse_file("test.py", "python", code)
    symbols = result.get('symbols', [])
    
    assert len(symbols) > 0
    
    # Check for function symbols
    function_symbols = [s for s in symbols if s.get('type') == 'function']
    assert len(function_symbols) > 0
    
    # Check for class symbols
    class_symbols = [s for s in symbols if s.get('type') == 'class']
    assert len(class_symbols) > 0


@pytest.mark.asyncio
async def test_incremental_update_unchanged_file(ast_engine):
    """Test incremental update with unchanged file content"""
    code = "def test_func(): pass"
    
    # First parse
    result1 = await ast_engine.parse_file("test.py", "python", code)
    symbols1 = result1.get('symbols', [])
    
    # Second parse with same content
    start_time = time.time()
    result2 = await ast_engine.parse_file("test.py", "python", code)
    symbols2 = result2.get('symbols', [])
    end_time = time.time()
    
    # Should return cached results quickly
    assert symbols1 == symbols2
    assert (end_time - start_time) < 0.1  # Should be very fast for cached results


@pytest.mark.asyncio
async def test_incremental_update_changed_file(ast_engine):
    """Test incremental update with changed file content"""
    original_code = "def func1(): pass\n\ndef func2(): pass"
    modified_code = "def func1(): pass\n\ndef func2(): pass\n\ndef func3(): pass"
    
    # First parse
    result1 = await ast_engine.parse_file("test.py", "python", original_code)
    symbols1 = result1.get('symbols', [])
    
    # Second parse with modified content
    start_time = time.time()
    result2 = await ast_engine.parse_file("test.py", "python", modified_code)
    symbols2 = result2.get('symbols', [])
    end_time = time.time()
    
    # Should have more symbols after modification
    assert len(symbols2) > len(symbols1)
    
    # Should complete reasonably quickly
    assert (end_time - start_time) < 1.0  # Should be under 1 second


@pytest.mark.asyncio
async def test_repository_wide_symbol_graph(ast_engine):
    """Test repository-wide symbol graph functionality"""
    # Parse multiple files
    await ast_engine.parse_file("file1.py", "python", "def func1(): pass")
    await ast_engine.parse_file("file2.py", "python", "def func2(): pass")
    await ast_engine.parse_file("file3.py", "python", "class MyClass: pass")
    
    # Get all symbols
    all_symbols = ast_engine.get_all_symbols()
    
    # Should have symbols from all files
    assert len(all_symbols) >= 3
    
    # Check if symbols are properly indexed
    symbol_names = [s.get('name') for s in all_symbols.values()]
    assert 'func1' in symbol_names
    assert 'func2' in symbol_names
    assert 'MyClass' in symbol_names


@pytest.mark.asyncio
async def test_symbol_search(ast_engine):
    """Test symbol search functionality"""
    # Parse some files with known symbols
    await ast_engine.parse_file("test1.py", "python", "def my_function(): pass")
    await ast_engine.parse_file("test2.py", "python", "def another_function(): pass")
    
    # Search for symbols
    results = ast_engine.search_symbols("function")
    
    # Should find matching symbols
    assert len(results) > 0
    
    # Check if results contain expected symbols
    result_names = [s.get('name') for s in results]
    assert any('function' in name for name in result_names)


@pytest.mark.asyncio
async def test_repository_stats(ast_engine):
    """Test repository statistics"""
    # Parse some files
    await ast_engine.parse_file("file1.py", "python", "def func1(): pass")
    await ast_engine.parse_file("file2.py", "python", "class MyClass: pass")
    
    # Get statistics
    stats = ast_engine.get_repository_stats()
    
    # Check statistics structure
    assert 'total_files' in stats
    assert 'total_symbols' in stats
    assert 'symbol_types' in stats
    assert 'language_distribution' in stats
    
    # Should have reasonable values
    assert stats['total_files'] >= 2
    assert stats['total_symbols'] >= 2
    assert 'function' in stats['symbol_types']
    assert 'class' in stats['symbol_types']


@pytest.mark.asyncio
async def test_file_symbols(ast_engine):
    """Test getting symbols for a specific file"""
    code = "def func1(): pass\n\ndef func2(): pass"
    
    await ast_engine.parse_file("test.py", "python", code)
    
    # Get symbols for the file
    symbols = ast_engine.get_file_symbols("test.py")
    
    # Should have symbols from the file
    assert len(symbols) >= 2
    
    # Check symbol names
    symbol_names = [s.get('name') for s in symbols]
    assert 'func1' in symbol_names
    assert 'func2' in symbol_names


@pytest.mark.asyncio
async def test_symbol_info(ast_engine):
    """Test getting symbol information"""
    code = "def my_function(): pass"
    
    await ast_engine.parse_file("test.py", "python", code)
    
    # Get symbol info
    symbol_info = ast_engine.get_symbol_info("my_function")
    
    # Should find the symbol
    assert symbol_info.get('name') == 'my_function'
    assert symbol_info.get('type') == 'function'
    assert symbol_info.get('file_path') == 'test.py'


@pytest.mark.asyncio
async def test_parsing_performance(ast_engine):
    """Test parsing performance meets requirements"""
    # Generate a larger code file
    code_lines = []
    for i in range(100):
        code_lines.append(f"def function_{i}():")
        code_lines.append(f"    return {i}")
        code_lines.append("")
    
    code = "\n".join(code_lines)
    
    # Measure parsing time
    start_time = time.time()
    result = await ast_engine.parse_file("large_file.py", "python", code)
    end_time = time.time()
    
    parsing_time = end_time - start_time
    symbols = result.get('symbols', [])
    
    # Should parse reasonably fast (under 5 seconds for 100 functions)
    assert parsing_time < 5.0
    assert len(symbols) >= 100


@pytest.mark.asyncio
async def test_multiple_language_support(ast_engine):
    """Test support for multiple programming languages"""
    # Test Python
    python_code = "def python_func(): pass"
    await ast_engine.parse_file("test.py", "python", python_code)
    
    # Test JavaScript
    js_code = "function jsFunction() {}"
    await ast_engine.parse_file("test.js", "javascript", js_code)
    
    # Test Rust
    rust_code = "fn rust_func() {}"
    await ast_engine.parse_file("test.rs", "rust", rust_code)
    
    # Get statistics
    stats = ast_engine.get_repository_stats()
    
    # Should have symbols from multiple languages
    assert stats['total_files'] >= 3
    
    # Check language distribution
    language_dist = stats['language_distribution']
    assert 'python' in language_dist
    assert 'javascript' in language_dist
    assert 'rust' in language_dist


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
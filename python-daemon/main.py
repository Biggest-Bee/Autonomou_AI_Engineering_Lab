#!/usr/bin/env python3
"""
Autonomous AI Engineering Lab - Python Background Daemon
Stage 2: AI Brain & IPC Layer

This daemon handles:
- Simple IPC with Rust host engine
- AST Repo-Map parsing engine
- In-memory Vector Database for local RAG
- Symbol extraction and code understanding
"""

import asyncio
import json
import signal
import sys
from typing import Dict, Any
import structlog

from ast_engine import ASTRepoMap
from vector_store import VectorStore
from ipc import SimpleIPC
from ml_inference import MLInferenceEngine

# Configure structured logging
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    cache_logger_on_first_use=True,
)

log = structlog.get_logger()


class AIBrainDaemon:
    """
    Main AI Brain Daemon - Handles background AI processing and IPC
    """
    
    def __init__(self):
        self.ast_engine = ASTRepoMap()
        self.vector_store = VectorStore()
        self.ipc = SimpleIPC()
        self.ml_engine = MLInferenceEngine()
        self.running = False
        
    async def initialize(self):
        """Initialize all AI Brain components"""
        log.info("initializing_ai_brain_daemon")

        # Initialize AST engine
        log.info("initializing_ast_engine")

        # Initialize Vector Store
        await self.vector_store.initialize()

        # Initialize ML Inference Engine
        await self.ml_engine.initialize()

        # Register IPC handlers
        self.ipc.register_handler('ast_update', self.process_ast_update)
        self.ipc.register_handler('search', self.search_similar_code)
        self.ipc.register_handler('symbol_info', self.get_symbol_info)
        self.ipc.register_handler('code_context', self.get_code_context)
        self.ipc.register_handler('stats', self.get_stats)
        self.ipc.register_handler('generate_code', self.generate_code)
        self.ipc.register_handler('analyze_code', self.analyze_code)
        self.ipc.register_handler('suggest_improvements', self.suggest_improvements)
        self.ipc.register_handler('ml_info', self.get_ml_info)

        log.info("ai_brain_daemon_initialized")
        
    async def process_ast_update(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """
        Process AST updates from the IDE
        Uses regex-based parsing to extract symbols and update symbol graph
        """
        file_path = message['file_path']
        language = message['language']
        code = message['code']
        
        log.info("processing_ast_update", file=file_path, language=language)
        
        try:
            # Parse code and extract symbols
            result = await self.ast_engine.parse_file(file_path, language, code)
            symbols = result.get('symbols', [])
            
            # Index symbols in vector store
            for symbol in symbols:
                code_context = symbol.get('signature', '')
                await self.vector_store.index_symbol(
                    symbol_name=symbol['name'],
                    file_path=file_path,
                    language=language,
                    code_context=code_context,
                    symbol_type=symbol['type']
                )
            
            log.info("ast_update_processed", file=file_path, symbol_count=len(symbols))
            
            return {
                'status': 'success',
                'symbols_extracted': len(symbols),
                'symbols': symbols
            }
            
        except Exception as e:
            log.error("ast_update_error", file=file_path, error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }
    
    async def search_similar_code(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """
        Search for similar code using semantic search
        """
        query = message['query']
        language = message.get('language')
        limit = message.get('limit', 10)
        
        log.info("searching_similar_code", query=query, language=language)
        
        try:
            results = await self.vector_store.search_similar_code(
                query=query,
                limit=limit,
                language_filter=language
            )
            
            log.info("search_completed", result_count=len(results))
            
            return {
                'status': 'success',
                'results': results,
                'count': len(results)
            }
            
        except Exception as e:
            log.error("search_error", query=query, error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }
    
    async def get_symbol_info(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Get information about a specific symbol"""
        symbol_name = message['symbol_name']
        
        log.info("getting_symbol_info", symbol=symbol_name)
        
        try:
            symbol_info = self.ast_engine.get_symbol_info(symbol_name)
            
            return {
                'status': 'success',
                'symbol_info': symbol_info
            }
            
        except Exception as e:
            log.error("symbol_info_error", symbol=symbol_name, error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }
    
    async def get_code_context(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Get code context for a specific location"""
        file_path = message['file_path']
        line = message['line']
        column = message['column']
        
        log.info("getting_code_context", file=file_path, line=line, column=column)
        
        try:
            context = await self.ast_engine.get_code_context(file_path, line, column)
            
            return {
                'status': 'success',
                'context': context
            }
            
        except Exception as e:
            log.error("context_error", file=file_path, error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }
    
    async def get_stats(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Get daemon statistics"""
        try:
            symbol_count = len(self.ast_engine.get_all_symbols())
            vector_stats = await self.vector_store.get_collection_stats()
            ml_info = await self.ml_engine.get_model_info()

            return {
                'status': 'success',
                'symbols_indexed': symbol_count,
                'vectors_stored': vector_stats['points_count'],
                'ml_status': ml_info['status'],
                'vector_backend': vector_stats['backend']
            }

        except Exception as e:
            log.error("stats_error", error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }

    async def generate_code(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Generate code using ML inference"""
        prompt = message['prompt']
        language = message.get('language', 'python')
        max_length = message.get('max_length', 200)

        log.info("generating_code", prompt_length=len(prompt), language=language)

        try:
            result = await self.ml_engine.generate_code(prompt, language, max_length)
            return result
        except Exception as e:
            log.error("code_generation_error", error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }

    async def analyze_code(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Analyze code using ML inference"""
        code = message['code']
        language = message.get('language', 'python')

        log.info("analyzing_code", code_length=len(code), language=language)

        try:
            result = await self.ml_engine.analyze_code(code, language)
            return result
        except Exception as e:
            log.error("code_analysis_error", error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }

    async def suggest_improvements(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Suggest code improvements using ML inference"""
        code = message['code']
        language = message.get('language', 'python')

        log.info("suggesting_improvements", code_length=len(code), language=language)

        try:
            result = await self.ml_engine.suggest_improvements(code, language)
            return result
        except Exception as e:
            log.error("improvements_error", error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }

    async def get_ml_info(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Get ML engine information"""
        try:
            info = await self.ml_engine.get_model_info()
            return {
                'status': 'success',
                'ml_info': info
            }
        except Exception as e:
            log.error("ml_info_error", error=str(e))
            return {
                'status': 'error',
                'error': str(e)
            }
    
    async def run(self):
        """Main daemon run loop"""
        log.info("starting_ai_brain_daemon")
        self.running = True
        
        # Start IPC loop
        await self.ipc.run()
        
        log.info("ai_brain_daemon_stopped")
        
    async def shutdown(self):
        """Graceful shutdown"""
        log.info("shutting_down_ai_brain_daemon")
        self.running = False
        self.ipc.stop()
        log.info("ai_brain_daemon_shutdown_complete")


async def main():
    """Main entry point"""
    daemon = AIBrainDaemon()
    
    # Setup signal handlers for graceful shutdown
    def signal_handler(signum, frame):
        log.info("received_signal", signal=signum)
        asyncio.create_task(daemon.shutdown())
        
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    try:
        await daemon.initialize()
        
        # Send initialization message
        await daemon.ipc.send_message({
            'type': 'ready',
            'status': 'daemon_ready'
        })
        
        await daemon.run()
    except Exception as e:
        log.error("daemon_error", error=str(e))
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
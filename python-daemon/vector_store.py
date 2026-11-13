"""
Vector Database Integration
Handles local RAG (Retrieval-Augmented Generation) for code search and context
"""

import asyncio
from typing import List, Dict, Any, Optional
import structlog
import numpy as np
from collections import defaultdict

log = structlog.get_logger()

# Try to import sentence-transformers, but handle gracefully if not available
try:
    from sentence_transformers import SentenceTransformer
    SENTENCE_TRANSFORMERS_AVAILABLE = True
except ImportError:
    SENTENCE_TRANSFORMERS_AVAILABLE = False
    log.warning("sentence_transformers_not_available", message="using hash-based embeddings as fallback")

# Try to import Qdrant client, but handle gracefully if not available
try:
    from qdrant_client import QdrantClient
    from qdrant_client.models import Distance, VectorParams, PointStruct, Filter, FieldCondition, MatchValue
    QDRANT_AVAILABLE = True
except ImportError:
    QDRANT_AVAILABLE = False
    log.warning("qdrant_not_available", message="using in-memory vector store as fallback")


class VectorStore:
    """
    Vector Store for code embeddings and semantic search
    Uses Qdrant for persistent vector storage with in-memory fallback
    Uses sentence-transformers for real embeddings with fallback to hash-based approach
    """

    def __init__(self):
        self.embeddings: Dict[str, np.ndarray] = {}
        self.metadata: Dict[str, Dict[str, Any]] = {}
        self.embedding_dim = 768
        self.embedding_model = None
        self.use_real_embeddings = False
        self.qdrant_client = None
        self.use_qdrant = False
        self.collection_name = "code_symbols"
        
    async def initialize(self):
        """Initialize vector store with Qdrant and real embedding model if available"""
        log.info("initializing_vector_store")

        # Initialize Qdrant client if available
        if QDRANT_AVAILABLE:
            try:
                # Connect to local Qdrant instance
                self.qdrant_client = QdrantClient(url="http://localhost:6333")
                self.use_qdrant = True

                # Create collection if it doesn't exist
                self._ensure_qdrant_collection()

                log.info("qdrant_client_initialized", collection=self.collection_name)
            except Exception as e:
                log.warning("qdrant_initialization_failed", error=str(e))
                self.use_qdrant = False
                log.info("using_in_memory_fallback")
        else:
            self.use_qdrant = False
            log.info("using_in_memory_fallback")

        # Initialize sentence-transformers if available
        if SENTENCE_TRANSFORMERS_AVAILABLE:
            try:
                # Load a pre-trained sentence transformer model
                # Using 'all-MiniLM-L6-v2' for good performance with reasonable size
                self.embedding_model = SentenceTransformer('all-MiniLM-L6-v2')
                self.embedding_dim = self.embedding_model.get_sentence_embedding_dimension()
                self.use_real_embeddings = True
                log.info("sentence_transformer_loaded", model="all-MiniLM-L6-v2", embedding_dim=self.embedding_dim)
            except Exception as e:
                log.warning("sentence_transformer_load_failed", error=str(e))
                self.use_real_embeddings = False
                log.info("using_fallback_hash_embeddings")
        else:
            self.use_real_embeddings = False
            log.info("using_fallback_hash_embeddings")

        log.info("vector_store_initialized",
                 backend="qdrant" if self.use_qdrant else "in_memory",
                 embeddings="real" if self.use_real_embeddings else "hash")

    def _ensure_qdrant_collection(self):
        """Ensure Qdrant collection exists"""
        try:
            collections = self.qdrant_client.get_collections().collections
            collection_exists = any(c.name == self.collection_name for c in collections)

            if not collection_exists:
                self.qdrant_client.create_collection(
                    collection_name=self.collection_name,
                    vectors_config=VectorParams(size=self.embedding_dim, distance=Distance.COSINE)
                )
                log.info("qdrant_collection_created", collection=self.collection_name)
        except Exception as e:
            log.warning("qdrant_collection_check_failed", error=str(e))
        
    async def index_symbol(self, symbol_name: str, file_path: str, language: str,
                          code_context: str, symbol_type: str) -> str:
        """
        Index a code symbol in the vector database
        Returns the point ID
        """
        log.info("indexing_symbol", symbol=symbol_name, file=file_path)

        # Generate embedding
        embedding = self._generate_embedding(code_context)

        # Create point ID
        point_id = f"{file_path}:{symbol_name}"

        if self.use_qdrant:
            try:
                # Index in Qdrant
                self.qdrant_client.upsert(
                    collection_name=self.collection_name,
                    points=[PointStruct(
                        id=point_id,
                        vector=embedding.tolist(),
                        payload={
                            'symbol_name': symbol_name,
                            'file_path': file_path,
                            'language': language,
                            'symbol_type': symbol_type,
                            'code_context': code_context
                        }
                    )]
                )
                log.info("symbol_indexed_qdrant", symbol=symbol_name, point_id=point_id)
            except Exception as e:
                log.warning("qdrant_index_failed", error=str(e), fallback="in_memory")
                # Fallback to in-memory
                self._index_in_memory(point_id, embedding, symbol_name, file_path, language, symbol_type, code_context)
        else:
            # Use in-memory storage
            self._index_in_memory(point_id, embedding, symbol_name, file_path, language, symbol_type, code_context)

        return point_id

    def _index_in_memory(self, point_id: str, embedding: np.ndarray, symbol_name: str,
                        file_path: str, language: str, symbol_type: str, code_context: str):
        """Index symbol in in-memory storage"""
        self.embeddings[point_id] = embedding
        self.metadata[point_id] = {
            'symbol_name': symbol_name,
            'file_path': file_path,
            'language': language,
            'symbol_type': symbol_type,
            'code_context': code_context
        }
        log.info("symbol_indexed_memory", symbol=symbol_name, point_id=point_id)
    
    def _generate_embedding(self, text: str) -> np.ndarray:
        """
        Generate embedding using sentence-transformers or fallback to hash-based approach
        """
        if self.use_real_embeddings and self.embedding_model:
            try:
                # Use sentence-transformers for real embeddings
                embedding = self.embedding_model.encode(text, convert_to_numpy=True)
                return embedding.astype(np.float32)
            except Exception as e:
                log.warning("real_embedding_generation_failed", error=str(e))
                # Fallback to hash-based if real embedding fails
                return self._generate_hash_embedding(text)
        else:
            # Use hash-based fallback
            return self._generate_hash_embedding(text)

    def _generate_hash_embedding(self, text: str) -> np.ndarray:
        """
        Generate a simple embedding using hash-based approach (fallback)
        """
        # Simple hash-based embedding for demonstration
        embedding = np.zeros(self.embedding_dim, dtype=np.float32)

        # Use character codes to generate pseudo-embeddings
        for i, char in enumerate(text[:self.embedding_dim]):
            embedding[i] = (ord(char) % 256) / 256.0

        # Pad if text is shorter
        if len(text) < self.embedding_dim:
            embedding[len(text):] = 0.5

        return embedding
    
    async def search_similar_code(self, query: str, limit: int = 10,
                                  language_filter: Optional[str] = None) -> List[Dict[str, Any]]:
        """
        Search for similar code using semantic search
        Returns list of similar code snippets
        """
        log.info("searching_similar_code", query=query, limit=limit)

        # Generate embedding for query
        query_embedding = self._generate_embedding(query)

        if self.use_qdrant:
            try:
                # Search in Qdrant
                search_filter = None
                if language_filter:
                    search_filter = Filter(
                        must=[
                            FieldCondition(
                                key="language",
                                match=MatchValue(value=language_filter)
                            )
                        ]
                    )

                search_results = self.qdrant_client.search(
                    collection_name=self.collection_name,
                    query_vector=query_embedding.tolist(),
                    limit=limit,
                    query_filter=search_filter
                )

                results = []
                for result in search_results:
                    results.append({
                        'score': result.score,
                        'symbol_name': result.payload.get('symbol_name', ''),
                        'file_path': result.payload.get('file_path', ''),
                        'language': result.payload.get('language', ''),
                        'symbol_type': result.payload.get('symbol_type', ''),
                        'code_context': result.payload.get('code_context', '')
                    })

                log.info("search_completed_qdrant", result_count=len(results))
                return results

            except Exception as e:
                log.warning("qdrant_search_failed", error=str(e), fallback="in_memory")
                # Fallback to in-memory search
                return self._search_in_memory(query_embedding, limit, language_filter)
        else:
            # Use in-memory search
            return self._search_in_memory(query_embedding, limit, language_filter)

    def _search_in_memory(self, query_embedding: np.ndarray, limit: int,
                         language_filter: Optional[str] = None) -> List[Dict[str, Any]]:
        """Search in in-memory storage"""
        results = []
        for point_id, embedding in self.embeddings.items():
            # Check language filter
            if language_filter and self.metadata[point_id]['language'] != language_filter:
                continue

            # Calculate cosine similarity
            similarity = self._cosine_similarity(query_embedding, embedding)

            results.append({
                'score': similarity,
                'symbol_name': self.metadata[point_id]['symbol_name'],
                'file_path': self.metadata[point_id]['file_path'],
                'language': self.metadata[point_id]['language'],
                'symbol_type': self.metadata[point_id]['symbol_type'],
                'code_context': self.metadata[point_id]['code_context']
            })

        # Sort by similarity and return top results
        results.sort(key=lambda x: x['score'], reverse=True)
        results = results[:limit]

        log.info("search_completed_memory", result_count=len(results))
        return results
    
    def _cosine_similarity(self, a: np.ndarray, b: np.ndarray) -> float:
        """Calculate cosine similarity between two vectors"""
        dot_product = np.dot(a, b)
        norm_a = np.linalg.norm(a)
        norm_b = np.linalg.norm(b)
        
        if norm_a == 0 or norm_b == 0:
            return 0.0
            
        return dot_product / (norm_a * norm_b)
    
    async def delete_symbol(self, symbol_name: str, file_path: str):
        """Delete a symbol from the vector database"""
        point_id = f"{file_path}:{symbol_name}"

        if self.use_qdrant:
            try:
                self.qdrant_client.delete(
                    collection_name=self.collection_name,
                    points_selector=[point_id]
                )
                log.info("symbol_deleted_qdrant", symbol=symbol_name, point_id=point_id)
            except Exception as e:
                log.warning("qdrant_delete_failed", error=str(e), fallback="in_memory")
                # Fallback to in-memory deletion
                self._delete_in_memory(point_id)
        else:
            self._delete_in_memory(point_id)

    def _delete_in_memory(self, point_id: str):
        """Delete symbol from in-memory storage"""
        if point_id in self.embeddings:
            del self.embeddings[point_id]
            del self.metadata[point_id]
        log.info("symbol_deleted_memory", point_id=point_id)
    
    async def get_code_embedding(self, code: str) -> List[float]:
        """Generate embedding for code snippet"""
        embedding = self._generate_embedding(code)
        return embedding.tolist()
    
    async def batch_index_symbols(self, symbols: List[Dict[str, Any]]):
        """
        Batch index multiple symbols for efficiency
        """
        log.info("batch_indexing_symbols", count=len(symbols))
        
        for symbol in symbols:
            await self.index_symbol(
                symbol['symbol_name'],
                symbol['file_path'],
                symbol['language'],
                symbol['code_context'],
                symbol['symbol_type']
            )
        
        log.info("batch_indexing_completed", indexed_count=len(symbols))
    
    async def get_collection_stats(self) -> Dict[str, Any]:
        """Get statistics about the vector collection"""
        if self.use_qdrant:
            try:
                collection_info = self.qdrant_client.get_collection(self.collection_name)
                return {
                    'points_count': collection_info.points_count,
                    'vectors_count': collection_info.points_count,
                    'indexed_vectors_count': collection_info.points_count,
                    'status': collection_info.status,
                    'backend': 'qdrant'
                }
            except Exception as e:
                log.warning("qdrant_stats_failed", error=str(e), fallback="in_memory")
                return self._get_memory_stats()
        else:
            return self._get_memory_stats()

    def _get_memory_stats(self) -> Dict[str, Any]:
        """Get in-memory storage statistics"""
        return {
            'points_count': len(self.embeddings),
            'vectors_count': len(self.embeddings),
            'indexed_vectors_count': len(self.embeddings),
            'backend': 'in_memory'
        }
    
    def get_all_symbols(self) -> List[Dict[str, Any]]:
        """Get all indexed symbols"""
        return list(self.metadata.values())
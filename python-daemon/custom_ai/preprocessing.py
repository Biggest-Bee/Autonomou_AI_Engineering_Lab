"""
Text Preprocessing Module
Provides text cleaning, basic tokenization, and preprocessing utilities
"""

import re
import string
from typing import List, Optional, Dict, Set
from collections import Counter
import structlog

log = structlog.get_logger()


class TextCleaner:
    """Text cleaning utilities"""
    
    @staticmethod
    def clean_text(text: str) -> str:
        """Basic text cleaning"""
        # Remove extra whitespace
        text = re.sub(r'\s+', ' ', text)
        # Remove special characters (keep basic punctuation)
        text = re.sub(r'[^\w\s\.\,\!\?\;\'\"\-\(\)]', '', text)
        # Strip leading/trailing whitespace
        text = text.strip()
        return text
    
    @staticmethod
    def remove_special_chars(text: str, keep_punctuation: bool = True) -> str:
        """Remove special characters"""
        if keep_punctuation:
            # Keep basic punctuation
            pattern = r'[^\w\s\.\,\!\?\;\'\"\-\(\)]'
        else:
            # Remove all non-alphanumeric
            pattern = r'[^\w\s]'
        
        return re.sub(pattern, '', text)
    
    @staticmethod
    def normalize_whitespace(text: str) -> str:
        """Normalize whitespace"""
        return re.sub(r'\s+', ' ', text).strip()
    
    @staticmethod
    def remove_numbers(text: str) -> str:
        """Remove numbers"""
        return re.sub(r'\d+', '', text)
    
    @staticmethod
    def lowercase(text: str) -> str:
        """Convert to lowercase"""
        return text.lower()
    
    @staticmethod
    def remove_punctuation(text: str) -> str:
        """Remove punctuation"""
        return text.translate(str.maketrans('', '', string.punctuation))
    
    @staticmethod
    def remove_urls(text: str) -> str:
        """Remove URLs"""
        url_pattern = r'http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\\(\\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+'
        return re.sub(url_pattern, '', text)
    
    @staticmethod
    def remove_emails(text: str) -> str:
        """Remove email addresses"""
        email_pattern = r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b'
        return re.sub(email_pattern, '', text)
    
    @staticmethod
    def clean_code(text: str) -> str:
        """Clean code-like text (preserve structure)"""
        # Remove extra blank lines
        text = re.sub(r'\n\s*\n', '\n\n', text)
        # Remove trailing whitespace from lines
        text = '\n'.join(line.rstrip() for line in text.split('\n'))
        return text


class BasicTokenizer:
    """Basic word-level tokenizer"""
    
    def __init__(self, lowercase: bool = True, remove_punctuation: bool = False):
        self.lowercase = lowercase
        self.remove_punctuation = remove_punctuation
        self.vocab: Dict[str, int] = {}
        self.reverse_vocab: Dict[int, str] = {}
        self.vocab_size = 0
        self.special_tokens: Dict[str, str] = {
            "<pad>": "<pad>",
            "<unk>": "<unk>",
            "<bos>": "<bos>",
            "<eos>": "<eos>"
        }
    
    def tokenize(self, text: str) -> List[str]:
        """Tokenize text into words"""
        if self.lowercase:
            text = text.lower()
        
        if self.remove_punctuation:
            text = TextCleaner.remove_punctuation(text)
        
        # Simple whitespace tokenization
        tokens = text.split()
        return tokens
    
    def build_vocab(self, texts: List[str], min_frequency: int = 2) -> None:
        """Build vocabulary from texts"""
        counter = Counter()
        
        for text in texts:
            tokens = self.tokenize(text)
            counter.update(tokens)
        
        # Add special tokens first
        self.vocab = {}
        idx = 0
        for token in self.special_tokens.values():
            self.vocab[token] = idx
            idx += 1
        
        # Add frequent tokens
        for token, count in counter.items():
            if count >= min_frequency and token not in self.vocab:
                self.vocab[token] = idx
                idx += 1
        
        self.reverse_vocab = {v: k for k, v in self.vocab.items()}
        self.vocab_size = len(self.vocab)
        
        log.info("vocab_built", vocab_size=self.vocab_size, min_frequency=min_frequency)
    
    def encode(self, text: str) -> List[int]:
        """Encode text to token IDs"""
        tokens = self.tokenize(text)
        return [self.vocab.get(token, self.vocab["<unk>"]) for token in tokens]
    
    def decode(self, token_ids: List[int]) -> str:
        """Decode token IDs to text"""
        tokens = [self.reverse_vocab.get(id, "<unk>") for id in token_ids]
        return " ".join(tokens)
    
    def get_vocab_size(self) -> int:
        """Get vocabulary size"""
        return self.vocab_size
    
    def save_vocab(self, path: str) -> None:
        """Save vocabulary to file"""
        with open(path, 'w', encoding='utf-8') as f:
            for token, idx in sorted(self.vocab.items(), key=lambda x: x[1]):
                f.write(f"{token}\t{idx}\n")
        log.info("vocab_saved", path=path)
    
    def load_vocab(self, path: str) -> None:
        """Load vocabulary from file"""
        self.vocab = {}
        self.reverse_vocab = {}
        
        with open(path, 'r', encoding='utf-8') as f:
            for line in f:
                token, idx = line.strip().split('\t')
                self.vocab[token] = int(idx)
                self.reverse_vocab[int(idx)] = token
        
        self.vocab_size = len(self.vocab)
        log.info("vocab_loaded", path=path, vocab_size=self.vocab_size)


class CharacterTokenizer:
    """Character-level tokenizer"""
    
    def __init__(self, special_tokens: Optional[Dict[str, str]] = None):
        self.vocab: Dict[str, int] = {}
        self.reverse_vocab: Dict[int, str] = {}
        self.vocab_size = 0
        self.special_tokens = special_tokens or {
            "<pad>": "<pad>",
            "<unk>": "<unk>",
            "<bos>": "<bos>",
            "<eos>": "<eos>"
        }
    
    def tokenize(self, text: str) -> List[str]:
        """Tokenize text into characters"""
        return list(text)
    
    def build_vocab(self, texts: List[str]) -> None:
        """Build character vocabulary"""
        chars = set()
        for text in texts:
            chars.update(self.tokenize(text))
        
        # Add special tokens first
        self.vocab = {}
        idx = 0
        for token in self.special_tokens.values():
            self.vocab[token] = idx
            idx += 1
        
        # Add characters
        for char in sorted(chars):
            if char not in self.vocab:
                self.vocab[char] = idx
                idx += 1
        
        self.reverse_vocab = {v: k for k, v in self.vocab.items()}
        self.vocab_size = len(self.vocab)
        
        log.info("char_vocab_built", vocab_size=self.vocab_size)
    
    def encode(self, text: str) -> List[int]:
        """Encode text to character IDs"""
        chars = self.tokenize(text)
        return [self.vocab.get(char, self.vocab["<unk>"]) for char in chars]
    
    def decode(self, char_ids: List[int]) -> str:
        """Decode character IDs to text"""
        chars = [self.reverse_vocab.get(id, "<unk>") for id in char_ids]
        return "".join(chars)


class SubwordTokenizer:
    """Simple subword tokenizer (basic implementation)"""
    
    def __init__(self, vocab_size: int = 10000):
        self.vocab_size = vocab_size
        self.vocab: Dict[str, int] = {}
        self.reverse_vocab: Dict[int, str] = {}
        self.merge_rules: List[tuple] = []
    
    def tokenize(self, text: str) -> List[str]:
        """Tokenize text using BPE-like approach"""
        # Start with character-level tokenization
        words = text.split()
        tokenized = []
        
        for word in words:
            # Simple character tokenization (full BPE implementation would be more complex)
            chars = list(word)
            tokenized.extend(chars)
            tokenized.append(" ")  # Add space marker
        
        return tokenized[:-1]  # Remove trailing space
    
    def learn_bpe(self, texts: List[str], num_merges: int = 1000) -> None:
        """Learn BPE merge rules (simplified)"""
        # Collect word frequencies
        word_freqs = Counter()
        for text in texts:
            words = text.split()
            word_freqs.update(words)
        
        # Initialize with character vocabulary
        vocab = Counter()
        for word, freq in word_freqs.items():
            chars = list(word)
            for char in chars:
                vocab[char] += freq
        
        # Learn merge rules (simplified)
        self.merge_rules = []
        for _ in range(num_merges):
            # Find most frequent pair
            pairs = Counter()
            for word, freq in word_freqs.items():
                chars = list(word)
                for i in range(len(chars) - 1):
                    pair = (chars[i], chars[i + 1])
                    pairs[pair] += freq
            
            if not pairs:
                break
            
            best_pair = pairs.most_common(1)[0][0]
            self.merge_rules.append(best_pair)
            
            # Update word representations
            new_word_freqs = Counter()
            for word, freq in word_freqs.items():
                new_word = self._apply_merge(word, best_pair)
                new_word_freqs[new_word] = freq
            word_freqs = new_word_freqs
        
        # Build vocabulary
        self.vocab = {}
        idx = 0
        for char in sorted(vocab.keys()):
            self.vocab[char] = idx
            idx += 1
        
        for pair in self.merge_rules:
            merged = "".join(pair)
            if merged not in self.vocab:
                self.vocab[merged] = idx
                idx += 1
        
        self.reverse_vocab = {v: k for k, v in self.vocab.items()}
        log.info("bpe_vocab_built", vocab_size=len(self.vocab), num_merges=num_merges)
    
    def _apply_merge(self, word: str, pair: tuple) -> str:
        """Apply a merge rule to a word"""
        chars = list(word)
        merged = []
        i = 0
        while i < len(chars):
            if i < len(chars) - 1 and (chars[i], chars[i + 1]) == pair:
                merged.append("".join(pair))
                i += 2
            else:
                merged.append(chars[i])
                i += 1
        return "".join(merged)
    
    def encode(self, text: str) -> List[int]:
        """Encode text using learned BPE"""
        words = text.split()
        token_ids = []
        
        for word in words:
            # Apply merge rules
            current = list(word)
            for pair in self.merge_rules:
                i = 0
                while i < len(current) - 1:
                    if (current[i], current[i + 1]) == pair:
                        current[i] = "".join(pair)
                        del current[i + 1]
                    else:
                        i += 1
            
            # Convert to IDs
            for token in current:
                token_ids.append(self.vocab.get(token, self.vocab.get("<unk>", 0)))
        
        return token_ids


def truncate_text(text: str, max_length: int, tokenizer: Optional[BasicTokenizer] = None) -> str:
    """Truncate text to maximum length"""
    if tokenizer:
        tokens = tokenizer.tokenize(text)
        if len(tokens) > max_length:
            tokens = tokens[:max_length]
            return " ".join(tokens)
    else:
        # Simple character truncation
        if len(text) > max_length:
            return text[:max_length]
    return text


def pad_sequences(sequences: List[List[int]], max_length: int, padding_value: int = 0) -> List[List[int]]:
    """Pad sequences to same length"""
    padded = []
    for seq in sequences:
        if len(seq) > max_length:
            padded.append(seq[:max_length])
        else:
            padded.append(seq + [padding_value] * (max_length - len(seq)))
    return padded


log.info("text_preprocessing_loaded", message="Text preprocessing module loaded successfully")
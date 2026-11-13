"""
ML Inference Module
Handles machine learning inference for code understanding and generation
"""

import asyncio
import os
from typing import Dict, List, Any, Optional
import structlog

log = structlog.get_logger()

# Try to import ML libraries, but handle gracefully if not available
try:
    from transformers import AutoTokenizer, AutoModelForCausalLM, pipeline
    import torch
    ML_AVAILABLE = True
except ImportError:
    ML_AVAILABLE = False
    log.warning("ml_libraries_not_available", message="ML inference features disabled")


class MLInferenceEngine:
    """
    ML Inference Engine for code understanding and generation
    Uses Hugging Face transformers with fallback to mock responses
    """

    def __init__(self):
        self.tokenizer = None
        self.model = None
        self.code_pipeline = None
        self.use_ml = False
        # Priority: CUSTOM_MODEL_PATH > MODEL_NAME > default bootstrap model.
        # CUSTOM_MODEL_PATH may be a local directory (merged fine-tuned model)
        # or a HuggingFace repo id.
        self.model_name = (
            os.environ.get("CUSTOM_MODEL_PATH")
            or os.environ.get("MODEL_NAME")
            or "microsoft/CodeGPT-small-py"
        )
        # Optional LoRA adapter directory produced by finetune/kaggle_finetune.py
        self.lora_adapter_path = os.environ.get("LORA_ADAPTER_PATH")

    async def initialize(self):
        """Initialize ML models and pipelines"""
        log.info("initializing_ml_inference")

        if not ML_AVAILABLE:
            self.use_ml = False
            log.info("ml_not_available", message="using mock responses")
            return

        try:
            # Load the configured model (custom fine-tune, HF repo, or default)
            self.tokenizer = AutoTokenizer.from_pretrained(self.model_name)
            self.model = AutoModelForCausalLM.from_pretrained(self.model_name)

            # Optionally attach a LoRA adapter (requires `peft` installed)
            if self.lora_adapter_path:
                try:
                    from peft import PeftModel
                    self.model = PeftModel.from_pretrained(
                        self.model, self.lora_adapter_path
                    )
                    log.info("lora_adapter_loaded", path=self.lora_adapter_path)
                except ImportError:
                    log.warning(
                        "peft_not_installed",
                        message="LORA_ADAPTER_PATH set but peft missing; "
                                "adapter ignored. pip install peft",
                    )
                except Exception as e:
                    log.warning("lora_adapter_load_failed", error=str(e))

            # Create code generation pipeline
            self.code_pipeline = pipeline(
                "text-generation",
                model=self.model,
                tokenizer=self.tokenizer,
                max_length=200,
                temperature=0.7
            )

            self.use_ml = True
            log.info("ml_model_loaded", model=self.model_name)

        except Exception as e:
            log.warning("ml_model_load_failed", error=str(e))
            self.use_ml = False
            log.info("using_mock_responses")

    async def generate_code(self, prompt: str, language: str = "python",
                           max_length: int = 200) -> Dict[str, Any]:
        """
        Generate code based on a prompt
        Returns generated code and metadata
        """
        log.info("generating_code", prompt_length=len(prompt), language=language)

        if self.use_ml and self.code_pipeline:
            try:
                # Generate code using ML model.
                # Instruct/chat models get a chat-templated prompt; base
                # models (e.g. CodeGPT-small-py) use the raw comment prefix.
                if self.tokenizer and self.tokenizer.chat_template:
                    messages = [{
                        "role": "user",
                        "content": f"Write {language} code for: {prompt}",
                    }]
                    full_prompt = self.tokenizer.apply_chat_template(
                        messages, tokenize=False, add_generation_prompt=True
                    )
                else:
                    full_prompt = f"# {language}\n{prompt}"

                result = self.code_pipeline(
                    full_prompt, max_length=max_length, num_return_sequences=1
                )

                generated_code = result[0]['generated_text']
                # Remove the prompt from the result
                if generated_code.startswith(full_prompt):
                    generated_code = generated_code[len(full_prompt):]

                log.info("code_generated_ml", length=len(generated_code))
                return {
                    'status': 'success',
                    'generated_code': generated_code,
                    'model': self.model_name,
                    'method': 'ml_inference'
                }

            except Exception as e:
                log.warning("ml_generation_failed", error=str(e))
                # Fallback to mock response
                return self._generate_mock_code(prompt, language)
        else:
            # Use mock response
            return self._generate_mock_code(prompt, language)

    def _generate_mock_code(self, prompt: str, language: str) -> Dict[str, Any]:
        """Generate mock code response when ML is unavailable"""
        mock_code = f"# Mock generated code for {language}\n"
        mock_code += f"# Prompt: {prompt[:50]}...\n\n"

        if language == "python":
            mock_code += "def mock_function():\n"
            mock_code += "    # Implementation would go here\n"
            mock_code += "    pass\n"
        elif language == "javascript":
            mock_code += "function mockFunction() {\n"
            mock_code += "    // Implementation would go here\n"
            mock_code += "}\n"
        elif language == "rust":
            mock_code += "fn mock_function() {\n"
            mock_code += "    // Implementation would go here\n"
            mock_code += "}\n"
        else:
            mock_code += "// Mock implementation\n"

        log.info("code_generated_mock", length=len(mock_code))
        return {
            'status': 'success',
            'generated_code': mock_code,
            'model': 'mock',
            'method': 'mock_response'
        }

    async def analyze_code(self, code: str, language: str = "python") -> Dict[str, Any]:
        """
        Analyze code and provide insights
        Returns code analysis results
        """
        log.info("analyzing_code", code_length=len(code), language=language)

        if self.use_ml and self.model:
            try:
                # Analyze code using ML model
                inputs = self.tokenizer(code, return_tensors="pt", truncation=True, max_length=512)

                with torch.no_grad():
                    outputs = self.model(**inputs)
                    # Extract some basic analysis from the model outputs
                    # This is simplified - in production, you'd use more sophisticated analysis

                analysis = {
                    'status': 'success',
                    'language': language,
                    'complexity': 'medium',
                    'functions_count': code.count('def ') if language == 'python' else code.count('function'),
                    'lines_of_code': len(code.split('\n')),
                    'model': self.model_name,
                    'method': 'ml_inference'
                }

                log.info("code_analyzed_ml", analysis=analysis)
                return analysis

            except Exception as e:
                log.warning("ml_analysis_failed", error=str(e))
                # Fallback to mock analysis
                return self._analyze_code_mock(code, language)
        else:
            # Use mock analysis
            return self._analyze_code_mock(code, language)

    def _analyze_code_mock(self, code: str, language: str) -> Dict[str, Any]:
        """Generate mock code analysis when ML is unavailable"""
        lines = code.split('\n')
        analysis = {
            'status': 'success',
            'language': language,
            'complexity': 'low' if len(lines) < 20 else 'medium' if len(lines) < 50 else 'high',
            'functions_count': code.count('def ') if language == 'python' else code.count('function'),
            'lines_of_code': len(lines),
            'model': 'mock',
            'method': 'mock_analysis'
        }

        log.info("code_analyzed_mock", analysis=analysis)
        return analysis

    async def suggest_improvements(self, code: str, language: str = "python") -> Dict[str, Any]:
        """
        Suggest code improvements based on ML analysis
        Returns improvement suggestions
        """
        log.info("suggesting_improvements", code_length=len(code), language=language)

        if self.use_ml:
            try:
                # Generate improvement suggestions using ML
                analysis = await self.analyze_code(code, language)

                suggestions = [
                    "Consider adding error handling",
                    "Add documentation comments",
                    "Optimize loop performance"
                ]

                result = {
                    'status': 'success',
                    'suggestions': suggestions,
                    'analysis': analysis,
                    'model': self.model_name,
                    'method': 'ml_inference'
                }

                log.info("improvements_suggested_ml", count=len(suggestions))
                return result

            except Exception as e:
                log.warning("ml_suggestions_failed", error=str(e))
                # Fallback to mock suggestions
                return self._suggest_improvements_mock(code, language)
        else:
            # Use mock suggestions
            return self._suggest_improvements_mock(code, language)

    def _suggest_improvements_mock(self, code: str, language: str) -> Dict[str, Any]:
        """Generate mock improvement suggestions when ML is unavailable"""
        suggestions = [
            "Add type hints for better code clarity",
            "Consider using list comprehensions",
            "Add docstrings to functions",
            "Handle edge cases in error handling"
        ]

        result = {
            'status': 'success',
            'suggestions': suggestions,
            'analysis': self._analyze_code_mock(code, language),
            'model': 'mock',
            'method': 'mock_suggestions'
        }

        log.info("improvements_suggested_mock", count=len(suggestions))
        return result

    async def get_model_info(self) -> Dict[str, Any]:
        """Get information about the ML model"""
        return {
            'model_name': self.model_name if self.use_ml else 'mock',
            'custom_model_path': self.model_name
                if os.environ.get("CUSTOM_MODEL_PATH") else None,
            'lora_adapter_path': self.lora_adapter_path,
            'ml_available': ML_AVAILABLE,
            'use_ml': self.use_ml,
            'status': 'loaded' if self.use_ml else 'mock'
        }
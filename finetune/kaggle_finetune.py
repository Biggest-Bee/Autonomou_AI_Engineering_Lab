"""
Kaggle Fine-Tuning Script for the Autonomous AI Engineering Lab
================================================================

Run this inside a Kaggle notebook (https://www.kaggle.com/code) with a
GPU accelerator enabled (T4 is enough for the default 1.5B model).

Steps:
  1. Create a Kaggle account (free) and accept the GPU quota terms.
  2. New Notebook -> Settings -> Accelerator -> GPU T4 x2 (or P100).
  3. Upload `train.jsonl` (built by prepare_dataset.py) as a dataset
     or into /kaggle/working via the file upload button.
  4. Paste this entire file into a notebook cell and run all.
  5. Download the `output/` folder from the Output panel (~1-3 GB).
  6. Put it somewhere local and set CUSTOM_MODEL_PATH in .env.

What it does:
  - QLoRA fine-tune (4-bit quantized base + LoRA adapters) of a code
    model on your JSONL instruction data. Memory-efficient: fits a T4.
  - Saves BOTH the LoRA adapter and a merged full model, so the daemon
    can load either one.
"""

import os
import json

# ---------------------------------------------------------------------------
# 0. Install dependencies (Kaggle images already have torch/transformers,
#    but peft/trl/bitsandbytes usually need installing)
#    Runs as `!pip` inside a notebook, subprocess when run as a script.
# ---------------------------------------------------------------------------
import subprocess
import sys
subprocess.check_call([
    sys.executable, "-m", "pip", "install", "-q", "-U",
    "peft", "trl", "datasets", "bitsandbytes", "accelerate",
])

# ---------------------------------------------------------------------------
# 1. Config -- edit these
# ---------------------------------------------------------------------------
BASE_MODEL = "Qwen/Qwen2.5-Coder-1.5B-Instruct"  # T4-friendly code model
# Alternatives that fit a T4 (16GB):
#   "deepseek-ai/deepseek-coder-1.3b-base"
#   "Qwen/Qwen2.5-Coder-0.5B-Instruct"            # tiny, CPU-trainable-ish
DATA_PATH = "/kaggle/working/train.jsonl"        # upload train.jsonl here
OUTPUT_DIR = "/kaggle/working/output"

EPOCHS = 3
BATCH_SIZE = 4
GRAD_ACCUM = 4          # effective batch = 16
LEARNING_RATE = 2e-4
MAX_LENGTH = 2048
LORA_R = 16
LORA_ALPHA = 32

# ---------------------------------------------------------------------------
# 2. Load and format the dataset
# ---------------------------------------------------------------------------
import torch
from datasets import load_dataset

dataset = load_dataset("json", data_files=DATA_PATH, split="train")
print(f"Loaded {len(dataset)} examples")


def to_chat(example):
    """Convert {instruction, input, output} -> chat-format text."""
    user = example["instruction"]
    if example.get("input"):
        user += "\n\n" + example["input"]
    return {
        "text": (
            f"<|im_start|>user\n{user}<|im_end|>\n"
            f"<|im_start|>assistant\n{example['output']}<|im_end|>"
        )
    }


dataset = dataset.map(to_chat)

# ---------------------------------------------------------------------------
# 3. Load base model in 4-bit (QLoRA) + tokenizer
# ---------------------------------------------------------------------------
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training

bnb_config = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_quant_type="nf4",
    bnb_4bit_compute_dtype=torch.bfloat16,
    bnb_4bit_use_double_quant=True,
)

tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL)
tokenizer.pad_token = tokenizer.eos_token

model = AutoModelForCausalLM.from_pretrained(
    BASE_MODEL,
    quantization_config=bnb_config,
    device_map="auto",
    torch_dtype=torch.bfloat16,
)
model.config.use_cache = False
model = prepare_model_for_kbit_training(model)

lora_config = LoraConfig(
    r=LORA_R,
    lora_alpha=LORA_ALPHA,
    lora_dropout=0.05,
    bias="none",
    task_type="CAUSAL_LM",
    target_modules=[
        "q_proj", "k_proj", "v_proj", "o_proj",
        "gate_proj", "up_proj", "down_proj",
    ],
)
model = get_peft_model(model, lora_config)
model.print_trainable_parameters()

# ---------------------------------------------------------------------------
# 4. Tokenize
# ---------------------------------------------------------------------------
def tokenize(example):
    return tokenizer(
        example["text"],
        truncation=True,
        max_length=MAX_LENGTH,
        padding="max_length",
    )


tokenized = dataset.map(tokenize, remove_columns=["text"])

# ---------------------------------------------------------------------------
# 5. Train with TRL SFTTrainer
# ---------------------------------------------------------------------------
from trl import SFTTrainer, SFTConfig

training_args = SFTConfig(
    output_dir=OUTPUT_DIR,
    num_train_epochs=EPOCHS,
    per_device_train_batch_size=BATCH_SIZE,
    gradient_accumulation_steps=GRAD_ACCUM,
    learning_rate=LEARNING_RATE,
    logging_steps=10,
    save_strategy="epoch",
    bf16=True,
    report_to="none",
    max_length=MAX_LENGTH,
    dataset_text_field=None,  # we already produced "input_ids"
    packing=False,
)

trainer = SFTTrainer(
    model=model,
    args=training_args,
    train_dataset=tokenized,
    processing_class=tokenizer,
)

trainer.train()

# ---------------------------------------------------------------------------
# 6. Save adapter AND merged model
# ---------------------------------------------------------------------------
import os

adapter_dir = os.path.join(OUTPUT_DIR, "adapter")
merged_dir = os.path.join(OUTPUT_DIR, "merged")
os.makedirs(adapter_dir, exist_ok=True)
os.makedirs(merged_dir, exist_ok=True)

model.save_pretrained(adapter_dir)
tokenizer.save_pretrained(adapter_dir)
print(f"LoRA adapter saved to {adapter_dir}")

# Merge for a standalone model the daemon can load directly
merged = model.merge_and_unload()
merged.save_pretrained(merged_dir)
tokenizer.save_pretrained(merged_dir)
print(f"Merged model saved to {merged_dir}")

print("Done. Download the 'output' folder from /kaggle/working/output")

# Custom AI Fine-Tuning Pipeline

This directory contains everything needed to fine-tune your own code model
on Kaggle's free GPU tier and load it into the daemon.

## Overview

```
prepare_dataset.py  ->  train.jsonl  ->  kaggle_finetune.py  ->  output/
   (local, free)        (upload)         (Kaggle T4, free)       (download)
                                                            |
                                              CUSTOM_MODEL_PATH in .env
```

## Step 1 — Build the dataset (local, free)

```bash
# From your code (style learning):
python finetune/prepare_dataset.py --code-dir ./src --code-dir ./frontend/src \
    --output train.jsonl

# Plus your accepted codegen episodes (prompt/code pairs):
python finetune/prepare_dataset.py --pairs episodes.jsonl --output train.jsonl

# Plus docs with code examples:
python finetune/prepare_dataset.py --docs ./docs --output train.jsonl
```

`episodes.jsonl` format (one per line):
```json
{"prompt": "write a python function that reverses a string", "code": "def reverse(s):\n    return s[::-1]\n"}
```

Tip: log prompts + the code you *accepted* from the app — that's your
highest-quality training signal.

## Step 2 — Fine-tune on Kaggle (free GPU)

1. Create a free account at https://www.kaggle.com
2. New Notebook → Settings → Accelerator → **GPU T4**
3. Upload `train.jsonl` to `/kaggle/working/`
4. Paste `kaggle_finetune.py` into a cell and **Run All**
   - Default base: `Qwen/Qwen2.5-Coder-1.5B-Instruct` (fits T4, ~3 hrs for ~5k examples)
   - Swap `BASE_MODEL` for `deepseek-ai/deepseek-coder-1.3b-base` etc.
5. Download the `output/` folder from the Output panel

Kaggle gives ~30 GPU-hours/week free. QLoRA means the 1.5B model trains
in 4-bit — no paid GPU needed.

## Step 3 — Load the custom model into the daemon

```bash
# Option A: merged model (simplest — already a full model dir)
#   put output/merged anywhere, e.g. C:\models\my-code-model
CUSTOM_MODEL_PATH=C:\models\my-code-model

# Option B: LoRA adapter only (needs base model + peft)
CUSTOM_MODEL_PATH=Qwen/Qwen2.5-Coder-1.5B-Instruct
LORA_ADAPTER_PATH=C:\models\my-code-model\adapter
```

Add these to `.env` (copy `.env.example`). The daemon reads them at startup.

## Optional deps for adapter mode

```bash
pip install peft
```

Merged-model mode needs nothing extra — just `transformers` + `torch`
(already in `requirements.txt`).

## Verifying it works

```bash
cd python-daemon
python -c "import asyncio; from ml_inference import MLInferenceEngine as E; \
    e=E(); asyncio.run(e.initialize()); \
    print(asyncio.run(e.get_model_info()))"
```

`custom_model_path` should show your path and `status: loaded`.
Then `generate_code` calls will use your model (chat-templated prompt).

## Cost

- Dataset prep: $0 (local)
- Kaggle T4 training: $0 (free quota, ~30h/week)
- Model storage: $0 (local disk, ~3 GB for 1.5B merged)

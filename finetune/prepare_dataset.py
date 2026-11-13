"""
Dataset Preparation for Custom AI Fine-Tuning

Converts local code and prompt/code pairs into instruction-tuning
JSONL format for the Kaggle fine-tuning notebook.

Input sources (any combination):
  1. A directory of source files -- each file becomes a completion
     example (the model learns to continue/write code in that style).
  2. A JSONL file of {"prompt": ..., "code": ...} pairs -- explicit
     instruction examples (e.g. logged codegen episodes you accepted).
  3. Markdown/text files with fenced code blocks -- extracted as pairs.

Usage:
  python prepare_dataset.py --code-dir ./my-projects --pairs episodes.jsonl \
      --output train.jsonl --max-chars 4000
"""

import argparse
import json
import os
import re
from pathlib import Path
from typing import Iterator

CODE_EXTENSIONS = {
    ".py", ".js", ".ts", ".tsx", ".jsx", ".rs", ".go", ".java",
    ".c", ".cpp", ".h", ".hpp", ".cs", ".rb", ".php", ".swift",
    ".kt", ".scala", ".sh", ".ps1", ".sql", ".html", ".css",
}

SKIP_DIRS = {
    "node_modules", ".git", "target", "dist", "build", "venv",
    ".venv", "__pycache__", ".idea", ".vscode", "vendor",
}

FENCE_RE = re.compile(r"```(?:\w+)?\n(.*?)```", re.DOTALL)


def iter_code_files(root: Path) -> Iterator[Path]:
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.suffix.lower() in CODE_EXTENSIONS:
            yield path


def file_to_example(path: Path, max_chars: int) -> dict | None:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    text = text.strip()
    if len(text) < 50 or len(text) > max_chars:
        return None
    language = path.suffix.lstrip(".")
    return {
        "instruction": f"Write {language} code in the style of this project.",
        "input": "",
        "output": f"```{language}\n{text}\n```",
    }


def pair_to_example(obj: dict, max_chars: int) -> dict | None:
    prompt = obj.get("prompt") or obj.get("instruction") or ""
    code = obj.get("code") or obj.get("output") or obj.get("completion") or ""
    if not prompt or not code or len(code) > max_chars:
        return None
    return {"instruction": prompt, "input": "", "output": code}


def doc_to_examples(path: Path, max_chars: int) -> Iterator[dict]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return
    for i, m in enumerate(FENCE_RE.finditer(text)):
        block = m.group(1).strip()
        if 50 <= len(block) <= max_chars:
            yield {
                "instruction": f"Documented code example {i + 1} from {path.name}",
                "input": "",
                "output": f"```\n{block}\n```",
            }


def main() -> None:
    ap = argparse.ArgumentParser(description="Build fine-tuning JSONL dataset")
    ap.add_argument("--code-dir", action="append", default=[],
                    help="Directory of source files (repeatable)")
    ap.add_argument("--pairs", action="append", default=[],
                    help="JSONL file(s) of {prompt, code} pairs (repeatable)")
    ap.add_argument("--docs", action="append", default=[],
                    help="Markdown/text files to mine for code blocks (repeatable)")
    ap.add_argument("--output", default="train.jsonl")
    ap.add_argument("--max-chars", type=int, default=4000)
    ap.add_argument("--max-examples", type=int, default=20000)
    args = ap.parse_args()

    examples: list[dict] = []

    for d in args.code_dir:
        root = Path(d)
        if not root.is_dir():
            print(f"[skip] not a directory: {d}")
            continue
        for f in iter_code_files(root):
            ex = file_to_example(f, args.max_chars)
            if ex:
                examples.append(ex)

    for p in args.pairs:
        path = Path(p)
        if not path.is_file():
            print(f"[skip] not a file: {p}")
            continue
        with path.open(encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    ex = pair_to_example(json.loads(line), args.max_chars)
                except json.JSONDecodeError:
                    continue
                if ex:
                    examples.append(ex)

    for d in args.docs:
        path = Path(d)
        if path.is_file():
            examples.extend(doc_to_examples(path, args.max_chars))
        elif path.is_dir():
            for f in path.rglob("*"):
                if f.is_file() and f.suffix.lower() in {".md", ".txt", ".rst"}:
                    examples.extend(doc_to_examples(f, args.max_chars))

    examples = examples[: args.max_examples]

    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as fh:
        for ex in examples:
            fh.write(json.dumps(ex, ensure_ascii=False) + "\n")

    print(f"Wrote {len(examples)} examples to {out}")
    print("Next: upload this file to the Kaggle notebook (see finetune/README.md)")


if __name__ == "__main__":
    main()

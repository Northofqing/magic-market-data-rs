#!/usr/bin/env python3
"""Compare benchmark JSON files and enforce a relative regression budget."""
import json, sys
from pathlib import Path

def main(old_path: str, new_path: str, budget: float = 0.05) -> int:
    old = json.loads(Path(old_path).read_text()); new = json.loads(Path(new_path).read_text())
    old_ns = float(old["ns_per_op"]); new_ns = float(new["ns_per_op"])
    regression = new_ns / old_ns - 1.0
    print(f"old_ns_per_op={old_ns:.4f} new_ns_per_op={new_ns:.4f} regression={regression * 100:.2f}% budget={budget * 100:.2f}%")
    return 0 if regression <= budget else 1

if __name__ == "__main__":
    if len(sys.argv) not in (3, 4): raise SystemExit("usage: compare.py old.json new.json [budget]")
    raise SystemExit(main(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) == 4 else 0.05))

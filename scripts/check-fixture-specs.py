#!/usr/bin/env python3
"""Verify built fixture interfaces through the real Lumen Lens WASM parser."""

import argparse
import json
from pathlib import Path
import subprocess
import sys


FIXTURES = {
    "sample_counter": {
        "functions": {"increment", "get", "reset"},
        "errors": {"CounterError": {"Overflow": 1}},
    },
    "sample_token": {
        "functions": {
            "initialize", "balance", "transfer", "approve", "allowance",
            "transfer_from", "burn", "burn_from", "name", "symbol",
            "decimals", "total_supply",
        },
        "errors": {"TokenError": {
            "AlreadyInitialized": 1, "NotInitialized": 2, "NegativeAmount": 3,
            "InsufficientBalance": 4, "Overflow": 5, "InvalidDecimals": 6,
            "InsufficientAllowance": 7, "InvalidExpiration": 8,
        }},
    },
}


def check(cli, directory):
    for name, expected in FIXTURES.items():
        result = subprocess.run(
            [cli, "inspect", "--wasm", str(directory / (name + ".wasm"))],
            check=True, capture_output=True, text=True, timeout=30,
        )
        spec = json.loads(result.stdout)
        functions = {function["name"] for function in spec["functions"]}
        if functions != expected["functions"]:
            raise ValueError(f"{name}: unexpected functions: {sorted(functions)}")
        errors = {
            enum["name"]: {case["name"]: case["value"] for case in enum["cases"]}
            for enum in spec["error_enums"]
        }
        if errors != expected["errors"]:
            raise ValueError(f"{name}: error codes changed: {errors}")
        if name == "sample_token":
            allowance = next(item for item in spec["structs"] if item["name"] == "AllowanceData")
            fields = {field["name"]: field["type_spec"] for field in allowance["fields"]}
            if fields != {"amount": {"kind": "i128"}, "expiration_ledger": {"kind": "u32"}}:
                raise ValueError(f"{name}: allowance schema changed: {fields}")
        print(f"{name}: {len(functions)} functions and typed errors verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", default="lumenlens", help="Lumen Lens CLI executable")
    parser.add_argument("--wasm-dir", type=Path, default=(
        Path(__file__).resolve().parent.parent / "target/wasm32-unknown-unknown/release"
    ))
    args = parser.parse_args()
    try:
        check(args.cli, args.wasm_dir)
    except (OSError, ValueError, KeyError, TypeError, StopIteration, subprocess.SubprocessError) as error:
        print(f"Fixture inspection failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

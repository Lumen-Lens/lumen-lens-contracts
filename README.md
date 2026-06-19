# lumen-lens-contracts

Sample Soroban contracts used as fixtures for the Lumen Lens SDK generator.

## Contracts

| Contract | Description |
|---|---|
| `sample-counter` | Simple increment/get/reset counter |
| `sample-token` | SEP-41-style fungible token |

## Build

```bash
cargo build --release --target wasm32-unknown-unknown
```

WASMs land in `target/wasm32-unknown-unknown/release/`.

## Test

```bash
cargo test
```

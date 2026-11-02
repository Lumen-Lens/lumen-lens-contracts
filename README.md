# lumen-lens-contracts

Executable Soroban contract fixtures for the [Lumen Lens SDK generator](https://github.com/Lumen-Lens/lumen-lens-backend).
The counter exercises integer arguments and typed errors; the fixed-supply token
adds addresses, strings, signed 128-bit amounts, structured allowances, events,
and authorization flows for generated clients.

## Contracts

| Contract | Public operations | Fixture coverage |
|---|---|---|
| `sample-counter` | `increment`, `get`, `reset` | `u32`, checked overflow, `CounterError` |
| `sample-token` | `initialize`, `balance`, `transfer`, `approve`, `allowance`, `transfer_from`, `burn`, `burn_from`, `name`, `symbol`, `decimals`, `total_supply` | Token flows, `Address`, `String`, `i128`, `AllowanceData`, `TokenError` |

These are development fixtures, not audited deployment templates. The token uses
the legacy Address-based interface and event shapes documented in
[Soroban SDK 21's token interface](https://docs.rs/soroban-sdk/21.7.7/src/soroban_sdk/token.rs.html).
It does not claim compatibility with newer muxed-address interfaces. There is no
post-initialization minting, admin clawback, or upgrade operation.

## Build and validate

Install Rust stable and the WASM target:

```bash
rustup target add wasm32-unknown-unknown
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --release --target wasm32-unknown-unknown --locked
```

`Cargo.lock` is committed so local and CI fixture builds use the same dependency
resolution (Soroban SDK 21.7.7). Release settings live at the workspace root and
apply to both contracts. Built files:

```text
target/wasm32-unknown-unknown/release/sample_counter.wasm
target/wasm32-unknown-unknown/release/sample_token.wasm
```

CI runs formatting, native contract tests, linting, and release WASM builds. The
`contract-wasms` artifact includes both binaries and `SHA256SUMS`; run
`sha256sum -c SHA256SUMS` from the extracted artifact directory to verify them.

## Token behavior

- `initialize` requires the initial holder's authorization, accepts a nonnegative
  supply and 0–18 decimals, and rejects subsequent initialization. All initial
  tokens go to that holder. A zero initial supply is supported.
- Transfers require the sender's authorization. Negative amounts and insufficient
  balances are errors. Self-transfers check funds and leave balances unchanged;
  zero transfers are valid. Transfers preserve total supply.
- `approve` requires the holder's authorization and **replaces** an allowance.
  Amounts must be nonnegative. Positive approvals require an expiration at or
  after the current ledger, within the network's maximum storage TTL. Zero
  approvals revoke the allowance, including with an expiration in the past.
- `allowance` returns zero for missing or expired entries. The expiration ledger
  itself is inclusive. Allowances use temporary storage with TTL extended through
  their expiration; changing the expiry does not revive an old approval.
- `transfer_from` and `burn_from` require the **spender's** authorization and
  consume allowance. A failed operation rolls back allowance and balance changes.
- `burn` requires the holder's authorization; both burn methods reduce supply
  and the holder's balance by the same nonnegative amount.
- Initialization emits `mint`; transfers emit `transfer`; approvals emit `approve`;
  burns emit `burn`, including for delegated operations. Failed invocations do
  not retain their state changes or events.

Token configuration and initialization state share instance storage. Balances
use persistent storage. These fixtures do not implement automatic renewal of
instance, code, or balance TTLs; long-lived deployments need a storage-lifetime
and restoration strategy. Allowance expiry is tested separately from archival.

### Stable errors

| Contract | Error | Code |
|---|---|---|
| Counter | `Overflow` | 1 |
| Token | `AlreadyInitialized` | 1 |
| Token | `NotInitialized` | 2 |
| Token | `NegativeAmount` | 3 |
| Token | `InsufficientBalance` | 4 |
| Token | `Overflow` | 5 |
| Token | `InvalidDecimals` | 6 |
| Token | `InsufficientAllowance` | 7 |
| Token | `InvalidExpiration` | 8 |

Both enums are included in contract specs for SDK consumers. Authorization
failures are host authorization errors, distinct from these contract errors.

## Generate SDK fixtures

With the backend CLI installed (`cargo install --path cli` in its repository),
run these commands from this repository. Adjust the sibling backend path if needed:

```bash
lumenlens inspect --wasm target/wasm32-unknown-unknown/release/sample_token.wasm
lumenlens generate \
  --wasm target/wasm32-unknown-unknown/release/sample_token.wasm \
  --language typescript \
  --templates-dir ../lumen-lens-backend/templates \
  --output ./sdk-out/token-ts
```

Verify both built interfaces using the real backend parser:

```bash
python3 scripts/check-fixture-specs.py \
  --cli ../lumen-lens-backend/target/debug/lumenlens
```

The check verifies public function sets, stable error names and codes, and the
allowance struct schema. It requires a built CLI and fails on parsing errors or
unexpected interface changes.

**Known backend limitation:** inspection passes with backend commit `3e3e162`,
but TypeScript SDK rendering currently fails because its template engine does not
register the `ts_type` filter used by its templates. The generation command above
documents the intended integration; successful end-to-end SDK rendering is not
claimed by this contracts revision.

The WASMs can also be uploaded to the Lumen Lens dashboard when a compatible HTTP
API is configured. They are fixtures for interface parsing and SDK generation;
compiling them is not a network deployment or an audit.

## Regression coverage

Tests exercise metadata and supply initialization, reinitialization rejection,
invalid inputs without partial writes, authorization failures, exact spender
authorization, self-transfers, maximum amounts, zero transfers, balance and supply
conservation, allowance replacement/revocation, expiry boundaries, maximum TTL,
delegated spending, rollback, burn accounting, and token event topics/data. The
counter tests cover accumulation, reset, zero input, overflow rollback, and reuse
after reset.

## Compatibility notes

This revision changes `increment` and token mutations to return typed contract
errors. Token metadata getters return `NotInitialized` before initialization.
Token configuration moved from separate persistent entries to instance storage.
Regenerate SDKs and use **fresh contract deployments** for these fixtures; this
repository does not provide a migration for previously deployed token storage.

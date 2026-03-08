# Bonding Curve SDK Extraction Checklist

Use this checklist when handing this repository to Jupiter or copying it into a standalone public repo.

## 1. Files To Include

Copy these files as a minimum handoff package:

- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `LICENSE`
- `CHANGELOG.md`
- `extraction_checklist.md`
- `tests/fixtures/mainnet_pool_8r9aukf8.b64`
- `src/jupiter_adapter.rs`
- `src/accounts.rs`
- `src/errors.rs`
- `src/fees.rs`
- `src/lib.rs`
- `src/math.rs`
- `src/quote.rs`
- `src/state.rs`

## 2. Cargo Metadata To Verify

Before delivery, verify these fields in `Cargo.toml`:

- `repository`
- `homepage`
- `documentation`
- `authors`, if you want explicit author metadata in the crate
- pinned dependency compatibility for `jupiter-amm-interface`

## 3. Repository Hygiene

Add or confirm the following repository-level files:

- `.gitignore`
- CI workflow that runs `cargo fmt --check` and `cargo test --locked`
- CI workflow that runs `cargo test --locked --features jupiter-adapter`
- any security or audit references you plan to share with Jupiter

Before delivery:

- confirm the license wording and copyright holder
- confirm the crate version and changelog entry
- confirm the README still matches the current integration boundary

## 4. README Expectations

Make sure the README clearly explains:

- what the crate owns
- what stays outside the crate
- the current referral quote behavior
- how Jupiter should call the SDK from an adapter
- which commands validate the crate locally

## 5. Validation Commands

Run these before handing the repository to Jupiter:

```bash
cargo fmt --check
cargo test --locked
cargo test --locked --features jupiter-adapter
cargo clippy --all-targets --all-features -- -D warnings
cargo package --allow-dirty
```

## 6. Jupiter Handoff Summary

Include these points in the handoff message:

- the crate is deterministic and does not perform network calls
- it owns pool parsing, quote math, fee logic, venue metadata, and ABI-order account metas
- it does not own loader registration or execution routing
- the compile-checked adapter lives in `src/jupiter_adapter.rs`
- the repository includes a real mainnet pool fixture for snapshot and adapter regression tests
- the SDK supports referral-aware quote math, but the current adapter keeps quote-time policy at no-referral until referrer context is available
- the adapter enforces exact-in mode for both quote and swap-meta paths
- adapter `update()` validates that the pool owner matches the bonding curve program id

## 7. Remaining Jupiter-Side Work

The receiving integration repo still needs to:

- enable, compile, and register `BondingCurveAmm`
- wire the `Swap::MeteoraDynamicBondingCurveSwapWithRemainingAccounts` execution path
- add execution or simulation parity tests

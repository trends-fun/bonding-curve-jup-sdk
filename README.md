# Bonding Curve SDK

Pure Rust quoting SDK for the Bonding Curve venue, prepared for Jupiter AMM integration.

## Status

This repository contains the SDK portion of the integration:

- deterministic pool parsing from account data
- exact-in quote math for both directions
- exact-in enforcement in both quote and swap-meta paths
- on-chain fee semantics, including referral-aware fee splitting
- venue metadata and PDA helpers
- swap account metas in contract ABI order
- adapter state updates that validate pool account ownership
- an optional compile-checked Jupiter adapter behind the `jupiter-adapter` feature

This repository does not yet contain a fully wired Jupiter integration:

- no loader registration
- no Jupiter execution-layer plumbing
- no route-execution or simulation-parity tests

## Design Goals

- easy for Jupiter to fork and maintain
- no network calls
- no RPC dependency
- deterministic quote behavior from account snapshot input only
- thin adapter layer in `jupiter-core`
- standalone-repo friendly packaging

## What The SDK Owns

- `PoolSnapshot` parsing from raw pool account data
- quote direction detection from input and output mints
- quote math for quote-to-base and base-to-quote swaps
- fee and market-cap calculations
- bonding curve program metadata and PDA helpers
- swap account meta construction

Main exports:

- `PoolSnapshot`
- `PoolSnapshot::try_from_account_data`
- `TradeDirection`
- `QuoteRequest`
- `QuoteResult`
- `quote`
- `quote_for_mints`
- `trade_direction_from_mints`
- `supports_mints`
- `calculate_market_cap`
- `calculate_fees`
- `BONDING_CURVE_PROGRAM_ID`
- `BONDING_CURVE_LABEL`
- `pool_authority`
- `config_address`
- `event_authority`
- `SwapAccountMetasParams`
- `build_swap_account_metas`

## What Stays Outside This Crate

- `QuoteParams` and `SwapParams` mapping (handled by the `jupiter-adapter` feature)
- placeholder account policy for optional referral accounts (handled by the `jupiter-adapter` feature)
- loader registration
- route execution and CPI plumbing
- Jupiter-side snapshot and execution tests

## Jupiter Integration Shape

The receiving Jupiter integration repo should:

1. depend on `bonding-curve-jup-sdk` with the `jupiter-adapter` feature enabled
2. register `BondingCurveAmm` in the loader or program-id map
3. wire `Swap::MeteoraDynamicBondingCurveSwapWithRemainingAccounts` to the execution path
4. add quote, account-meta, snapshot, and execution tests

Current adapter assumptions:

- `jupiter_amm_interface` is available in the target repo
- this crate currently pins `jupiter-amm-interface` to `=0.6.1`
- `Swap::MeteoraDynamicBondingCurveSwapWithRemainingAccounts` exists in the target execution path
- the target repo wants a deterministic no-referral quote policy until quote-time referrer context is available

## Jupiter Adapter Feature

Enable the adapter with:

```bash
cargo test --features jupiter-adapter
```

The feature exports `bonding_curve_sdk::BondingCurveAmm` and compile-checks the adapter
against `jupiter-amm-interface`.

Current test coverage includes a real mainnet pool fixture for:

- `PoolSnapshot::try_from_account_data`
- adapter quote output on real account bytes
- adapter swap account-meta shape with referral context
- adapter `update()` state refresh behavior
- adapter `ExactOut` rejection on swap-meta construction
- adapter `update()` rejection for unexpected account owner

## Minimal Adapter Flow

```rust
use bonding_curve_sdk::{quote_for_mints, PoolSnapshot, get_fee_denominator};

let snapshot = PoolSnapshot::try_from_account_data(&pool_account.data)?;
let sdk_quote = quote_for_mints(
    &snapshot,
    input_mint,
    output_mint,
    amount_in,
    false,
)?;

let bps = sdk_quote.fee_breakdown.creator_fee_bps.saturating_add(sdk_quote.fee_breakdown.protocol_fee_bps);

let jupiter_quote = jupiter_amm_interface::Quote {
    in_amount: amount_in,
    out_amount: sdk_quote.amount_out,
    fee_amount: sdk_quote.fee_amount,
    fee_mint: sdk_quote.fee_mint,
    fee_pct: rust_decimal::Decimal::from(bps)
        / rust_decimal::Decimal::from(get_fee_denominator()),
};
```

The source of truth for the adapter lives in `src/jupiter_adapter.rs`.

## Referral Behavior

- the SDK supports no-referral and referral-aware quote math
- the total fee remains the same when a referral is present
- the referral changes the split of the protocol fee, not the total fee charged
- the current adapter defaults `Amm::quote()` to no-referral because `QuoteParams` does not carry referrer presence
- swap execution can still pass a referral token account through `SwapParams::quote_mint_to_referrer`

## Validation

Run these locally before handing the repository to Jupiter:

```bash
cargo fmt --check
cargo test --locked
cargo test --locked --features jupiter-adapter
cargo clippy --all-targets --all-features -- -D warnings
cargo package --allow-dirty
```

The fixture currently snapshots mainnet pool `8r9aukF8nPpk33R7eTZW7nkVuLq2jrENVyvKmoSNoHfU`.

## Repository Guide

- [extraction_checklist.md](./extraction_checklist.md): files and metadata to include when handing this repo to Jupiter
- `src/jupiter_adapter.rs`: feature-backed Jupiter adapter implementation

## Delivery Notes

Before external handoff:

- verify `repository`, `homepage`, and `documentation` in `Cargo.toml` are correct
- verify the pinned `jupiter-amm-interface` version matches the target integration repo
- confirm CI continues to run formatting and tests
- confirm the license header and copyright holder
- keep a changelog for externally shared revisions

This crate is intended to satisfy the SDK side of Jupiter's DEX integration expectations: forkable, deterministic, and free of runtime network access.

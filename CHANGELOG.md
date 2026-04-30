# Changelog

All notable changes to this repository will be documented in this file.

## 0.2.0

### Breaking changes

- `PoolSnapshot` now includes `is_migrated`
- `QuoteError` now includes `MissingU8Byte`, `PoolCompleted`, and `PoolMigrated`

### Migration

- Update any downstream `PoolSnapshot { ... }` struct literals to include `is_migrated`
- Update exhaustive matches on `QuoteError` to handle the new variants

- Initial extraction of the Bonding Curve SDK for Jupiter AMM integration
- Pool snapshot parsing, fee logic, quote math, and swap account metas
- Feature-backed Jupiter adapter with compile-time interface checks
- Real mainnet pool fixture coverage for snapshot parsing, adapter quotes, metas, and updates
- Documentation cleanup for repository scope, delivery workflow, and adapter usage
- Enforced `ExactIn` mode in adapter swap-meta construction to match quote-mode constraints
- Added owner validation on adapter `update()` and regression tests for `ExactOut` + owner mismatch rejection
- Removed unused `UnsupportedDirection` error variant
- Fixed strict `clippy -D warnings` compatibility in fee-tier tests
- Included fixture files in crate packaging and filled Cargo package metadata links
- Pinned `jupiter-amm-interface` dependency to `=0.6.1` for integration consistency
- Disabled quoting for completed or migrated curve pools and added a `devnet` feature for threshold parity
- Capped quote-to-base buy quotes at the remaining migration capacity and surfaced the capped fill
  through `QuoteResult.amount_in` / Jupiter `Quote.in_amount`

use crate::{
    calculate_fees, calculate_market_cap,
    math::{checked_add_u128, checked_mul_u128, checked_sub, try_u64},
    FeeBreakdown, PoolSnapshot, QuoteError, WSOL_MINT,
};
use solana_pubkey::Pubkey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeDirection {
    QuoteToBase,
    BaseToQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteRequest {
    pub amount_in: u64,
    pub direction: TradeDirection,
    pub has_referral: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteResult {
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub fee_mint: Pubkey,
    pub fee_breakdown: FeeBreakdown,
    pub market_cap: u64,
}

pub fn supports_mints(snapshot: &PoolSnapshot, input_mint: Pubkey, output_mint: Pubkey) -> bool {
    (input_mint == WSOL_MINT && output_mint == snapshot.base_mint)
        || (input_mint == snapshot.base_mint && output_mint == WSOL_MINT)
}

pub fn trade_direction_from_mints(
    snapshot: &PoolSnapshot,
    input_mint: Pubkey,
    output_mint: Pubkey,
) -> Result<TradeDirection, QuoteError> {
    if input_mint == WSOL_MINT && output_mint == snapshot.base_mint {
        Ok(TradeDirection::QuoteToBase)
    } else if input_mint == snapshot.base_mint && output_mint == WSOL_MINT {
        Ok(TradeDirection::BaseToQuote)
    } else {
        Err(QuoteError::UnsupportedPair)
    }
}

pub fn quote_for_mints(
    snapshot: &PoolSnapshot,
    input_mint: Pubkey,
    output_mint: Pubkey,
    amount_in: u64,
    has_referral: bool,
) -> Result<QuoteResult, QuoteError> {
    let direction = trade_direction_from_mints(snapshot, input_mint, output_mint)?;
    quote(
        snapshot,
        QuoteRequest {
            amount_in,
            direction,
            has_referral,
        },
    )
}

pub fn quote(snapshot: &PoolSnapshot, request: QuoteRequest) -> Result<QuoteResult, QuoteError> {
    match request.direction {
        TradeDirection::QuoteToBase => {
            quote_quote_to_base(snapshot, request.amount_in, request.has_referral)
        }
        TradeDirection::BaseToQuote => {
            quote_base_to_quote(snapshot, request.amount_in, request.has_referral)
        }
    }
}

pub fn quote_quote_to_base(
    snapshot: &PoolSnapshot,
    amount_in: u64,
    has_referral: bool,
) -> Result<QuoteResult, QuoteError> {
    if amount_in == 0 {
        return Err(QuoteError::InvalidZeroAmount);
    }
    ensure_pool_is_tradeable(snapshot)?;
    ensure_virtual_liquidity(snapshot)?;

    let market_cap = calculate_market_cap(snapshot)?;
    let fee_breakdown = calculate_fees(market_cap, amount_in, has_referral)?;
    let actual_amount_in = checked_sub(amount_in, fee_breakdown.total_fee)?;

    let numerator = checked_mul_u128(
        u128::from(actual_amount_in),
        u128::from(snapshot.virtual_base_reserve),
    )?;
    let denominator = checked_add_u128(
        u128::from(snapshot.virtual_quote_reserve),
        u128::from(actual_amount_in),
    )?;

    let amount_out = try_u64(numerator / denominator)?;
    if amount_out > snapshot.base_reserve {
        return Err(QuoteError::InsufficientBaseReserve);
    }

    Ok(QuoteResult {
        amount_in,
        amount_out,
        fee_amount: fee_breakdown.total_fee,
        fee_mint: WSOL_MINT,
        fee_breakdown,
        market_cap,
    })
}

pub fn quote_base_to_quote(
    snapshot: &PoolSnapshot,
    amount_in: u64,
    has_referral: bool,
) -> Result<QuoteResult, QuoteError> {
    if amount_in == 0 {
        return Err(QuoteError::InvalidZeroAmount);
    }
    ensure_pool_is_tradeable(snapshot)?;
    ensure_virtual_liquidity(snapshot)?;

    let market_cap = calculate_market_cap(snapshot)?;
    let numerator = checked_mul_u128(
        u128::from(amount_in),
        u128::from(snapshot.virtual_quote_reserve),
    )?;
    let denominator = checked_add_u128(
        u128::from(snapshot.virtual_base_reserve),
        u128::from(amount_in),
    )?;
    let gross_out = try_u64(numerator / denominator)?;
    if gross_out > snapshot.quote_reserve {
        return Err(QuoteError::InsufficientQuoteReserve);
    }

    let fee_breakdown = calculate_fees(market_cap, gross_out, has_referral)?;
    let amount_out = checked_sub(gross_out, fee_breakdown.total_fee)?;

    Ok(QuoteResult {
        amount_in,
        amount_out,
        fee_amount: fee_breakdown.total_fee,
        fee_mint: WSOL_MINT,
        fee_breakdown,
        market_cap,
    })
}

fn ensure_virtual_liquidity(snapshot: &PoolSnapshot) -> Result<(), QuoteError> {
    if snapshot.virtual_base_reserve == 0 || snapshot.virtual_quote_reserve == 0 {
        return Err(QuoteError::ZeroLiquidity);
    }

    Ok(())
}

fn ensure_pool_is_tradeable(snapshot: &PoolSnapshot) -> Result<(), QuoteError> {
    if snapshot.is_migrated != 0 {
        return Err(QuoteError::PoolMigrated);
    }
    if snapshot.is_completed() {
        return Err(QuoteError::PoolCompleted);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_pubkey::Pubkey;

    fn snapshot() -> PoolSnapshot {
        PoolSnapshot {
            base_mint: Pubkey::new_unique(),
            base_vault: Pubkey::new_unique(),
            quote_vault: Pubkey::new_unique(),
            base_reserve: 1_000_000_000_000_000,
            quote_reserve: 5_000_000_000,
            virtual_base_reserve: 1_000_000_000_000_000,
            virtual_quote_reserve: 20_000_000_000,
            is_migrated: 0,
        }
    }

    #[test]
    fn quotes_quote_to_base() {
        let result = quote_quote_to_base(&snapshot(), 1_000_000, false).unwrap();

        assert_eq!(result.amount_out, 48_997_599_117);
        assert_eq!(result.fee_amount, 20_000);
        assert_eq!(result.fee_mint, WSOL_MINT);
        assert_eq!(result.fee_breakdown.creator_fee, 10_000);
        assert_eq!(result.fee_breakdown.protocol_fee, 10_000);
    }

    #[test]
    fn quotes_base_to_quote() {
        let result = quote_base_to_quote(&snapshot(), 1_000_000, false).unwrap();

        assert_eq!(result.amount_out, 19);
        assert_eq!(result.fee_amount, 0);
        assert_eq!(result.fee_mint, WSOL_MINT);
    }

    #[test]
    fn supports_referral_quote_to_base() {
        let result = quote_quote_to_base(&snapshot(), 1_000_000, true).unwrap();
        assert_eq!(result.fee_breakdown.creator_fee, 10_000);
        assert_eq!(result.fee_breakdown.protocol_fee, 7_000);
        assert_eq!(result.fee_breakdown.referral_fee, 3_000);
        assert_eq!(result.fee_amount, 20_000);
    }

    #[test]
    fn supports_referral_base_to_quote() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = 1_000_000_000;

        let result = quote_base_to_quote(&snapshot, 1_000_000_000_000, true).unwrap();
        assert_eq!(result.amount_out, 19_580_419);
        assert_eq!(result.fee_breakdown.creator_fee, 199_800);
        assert_eq!(result.fee_breakdown.protocol_fee, 139_860);
        assert_eq!(result.fee_breakdown.referral_fee, 59_940);
        assert_eq!(result.fee_amount, 399_600);
    }

    #[test]
    fn base_to_quote_fees_are_charged_on_gross_output() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = 1_000_000_000;

        let result = quote_base_to_quote(&snapshot, 1_000_000_000_000, false).unwrap();
        let gross_output = result.amount_out + result.fee_amount;
        let expected_fees = calculate_fees(result.market_cap, gross_output, false).unwrap();

        assert_eq!(result.fee_breakdown, expected_fees);
        assert_eq!(result.amount_out, gross_output - expected_fees.total_fee);
    }

    #[test]
    fn rejects_zero_virtual_liquidity() {
        let mut snapshot = snapshot();
        snapshot.virtual_base_reserve = 0;

        let err = quote(
            &snapshot,
            QuoteRequest {
                amount_in: 1_000_000,
                direction: TradeDirection::QuoteToBase,
                has_referral: false,
            },
        )
        .unwrap_err();

        assert_eq!(err, QuoteError::ZeroLiquidity);
    }

    #[test]
    fn rejects_completed_pool() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = crate::MIGRATION_QUOTE_THRESHOLD;

        let err = quote_quote_to_base(&snapshot, 1_000_000, false).unwrap_err();
        assert_eq!(err, QuoteError::PoolCompleted);
    }

    #[test]
    fn rejects_migrated_pool() {
        let mut snapshot = snapshot();
        snapshot.is_migrated = 1;

        let err = quote_base_to_quote(&snapshot, 1_000_000, false).unwrap_err();
        assert_eq!(err, QuoteError::PoolMigrated);
    }

    #[test]
    fn rejects_insufficient_base_reserve() {
        let mut snapshot = snapshot();
        snapshot.base_reserve = 1;

        let err = quote_quote_to_base(&snapshot, 1_000_000, false).unwrap_err();
        assert_eq!(err, QuoteError::InsufficientBaseReserve);
    }

    #[test]
    fn rejects_insufficient_quote_reserve() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = 1;

        let err = quote_base_to_quote(&snapshot, 1_000_000, false).unwrap_err();
        assert_eq!(err, QuoteError::InsufficientQuoteReserve);
    }

    #[test]
    fn derives_direction_from_mints() {
        let snapshot = snapshot();
        assert_eq!(
            trade_direction_from_mints(&snapshot, WSOL_MINT, snapshot.base_mint).unwrap(),
            TradeDirection::QuoteToBase
        );
        assert_eq!(
            trade_direction_from_mints(&snapshot, snapshot.base_mint, WSOL_MINT).unwrap(),
            TradeDirection::BaseToQuote
        );
    }

    #[test]
    fn rejects_unsupported_mint_pair() {
        let snapshot = snapshot();
        let err = trade_direction_from_mints(&snapshot, Pubkey::new_unique(), Pubkey::new_unique())
            .unwrap_err();
        assert_eq!(err, QuoteError::UnsupportedPair);
    }

    #[test]
    fn quotes_directly_from_mints() {
        let snapshot = snapshot();
        let result =
            quote_for_mints(&snapshot, WSOL_MINT, snapshot.base_mint, 1_000_000, false).unwrap();

        assert_eq!(result.amount_out, 48_997_599_117);
    }

    #[test]
    fn quotes_directly_from_base_to_quote_mints() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = 1_000_000_000;

        let result =
            quote_for_mints(&snapshot, snapshot.base_mint, WSOL_MINT, 1_000_000, false).unwrap();

        assert_eq!(result.amount_out, 19);
        assert_eq!(result.fee_mint, WSOL_MINT);
    }

    #[test]
    fn quote_dispatch_matches_direction_specific_paths() {
        let mut snapshot = snapshot();
        snapshot.quote_reserve = 1_000_000_000;

        let quote_to_base = quote(
            &snapshot,
            QuoteRequest {
                amount_in: 1_000_000,
                direction: TradeDirection::QuoteToBase,
                has_referral: true,
            },
        )
        .unwrap();
        assert_eq!(
            quote_to_base,
            quote_quote_to_base(&snapshot, 1_000_000, true).unwrap()
        );

        let base_to_quote = quote(
            &snapshot,
            QuoteRequest {
                amount_in: 1_000_000_000_000,
                direction: TradeDirection::BaseToQuote,
                has_referral: true,
            },
        )
        .unwrap();
        assert_eq!(
            base_to_quote,
            quote_base_to_quote(&snapshot, 1_000_000_000_000, true).unwrap()
        );
    }

    #[test]
    fn rejects_zero_amount_quote_to_base() {
        let err = quote_quote_to_base(&snapshot(), 0, false).unwrap_err();
        assert_eq!(err, QuoteError::InvalidZeroAmount);
    }

    #[test]
    fn rejects_zero_amount_base_to_quote() {
        let err = quote_base_to_quote(&snapshot(), 0, false).unwrap_err();
        assert_eq!(err, QuoteError::InvalidZeroAmount);
    }
}

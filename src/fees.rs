use crate::{
    math::{checked_add, checked_mul_u128, try_u64},
    PoolSnapshot, QuoteError,
};

const BASE_SUPPLY: u64 = 1_000_000_000_000_000;
const FEE_DENOMINATOR: u16 = 10_000;
const FEE_TIERS: &[(u64, u16, u16)] = &[
    (800_000_000_000, 100, 100),
    (4_000_000_000_000, 95, 95),
    (12_000_000_000_000, 90, 90),
    (24_000_000_000_000, 85, 85),
    (40_000_000_000_000, 80, 80),
    (64_000_000_000_000, 75, 75),
    (120_000_000_000_000, 70, 70),
    (200_000_000_000_000, 65, 65),
    (280_000_000_000_000, 60, 60),
    (400_000_000_000_000, 55, 55),
];
const DEFAULT_CREATOR_FEE: u16 = 50;
const DEFAULT_PROTOCOL_FEE: u16 = 50;
const REFERRAL_FEE_TIERS: &[(u64, u16)] =
    &[(12_000_000_000_000, 3000), (400_000_000_000_000, 2000)];
const DEFAULT_REFERRAL_FEE: u16 = 1500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeBreakdown {
    pub creator_fee: u64,
    pub protocol_fee: u64,
    pub referral_fee: u64,
    pub total_fee: u64,
    pub creator_fee_bps: u16,
    pub protocol_fee_bps: u16,
}

pub fn calculate_market_cap(snapshot: &PoolSnapshot) -> Result<u64, QuoteError> {
    if snapshot.virtual_base_reserve == 0 || snapshot.virtual_quote_reserve == 0 {
        return Err(QuoteError::ZeroLiquidity);
    }

    let market_cap = checked_mul_u128(
        u128::from(snapshot.virtual_quote_reserve),
        u128::from(BASE_SUPPLY),
    )?;

    try_u64(market_cap / u128::from(snapshot.virtual_base_reserve))
}

pub fn get_fee_denominator() -> u16 {
    FEE_DENOMINATOR
}

pub fn get_fee_rates(market_cap: u64) -> (u16, u16) {
    for tier in FEE_TIERS {
        if market_cap <= tier.0 {
            return (tier.1, tier.2);
        }
    }

    (DEFAULT_CREATOR_FEE, DEFAULT_PROTOCOL_FEE)
}

pub fn get_referral_fee_rate(market_cap: u64) -> u16 {
    for tier in REFERRAL_FEE_TIERS {
        if market_cap <= tier.0 {
            return tier.1;
        }
    }

    DEFAULT_REFERRAL_FEE
}

pub fn calculate_fees(
    market_cap: u64,
    amount: u64,
    has_referral: bool,
) -> Result<FeeBreakdown, QuoteError> {
    let (creator_fee_bps, protocol_fee_bps) = get_fee_rates(market_cap);
    let creator_fee = calc_fee(amount, creator_fee_bps)?;
    let base_protocol_fee = calc_fee(amount, protocol_fee_bps)?;
    let (protocol_fee, referral_fee) = if has_referral {
        let referral_fee_rate = get_referral_fee_rate(market_cap);
        let referral_fee = calc_fee(base_protocol_fee, referral_fee_rate)?;
        (base_protocol_fee.saturating_sub(referral_fee), referral_fee)
    } else {
        (base_protocol_fee, 0)
    };
    let total_fee = checked_add(creator_fee, protocol_fee)?;
    let total_fee = checked_add(total_fee, referral_fee)?;

    Ok(FeeBreakdown {
        creator_fee,
        protocol_fee,
        referral_fee,
        total_fee,
        creator_fee_bps,
        protocol_fee_bps,
    })
}

fn calc_fee(amount: u64, fee_rate: u16) -> Result<u64, QuoteError> {
    try_u64(
        checked_mul_u128(u128::from(amount), u128::from(fee_rate))? / u128::from(FEE_DENOMINATOR),
    )
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
    fn computes_market_cap() {
        assert_eq!(calculate_market_cap(&snapshot()).unwrap(), 20_000_000_000);
    }

    #[test]
    fn switches_fee_tiers_at_boundaries() {
        assert_eq!(get_fee_rates(800_000_000_000), (100, 100));
        assert_eq!(get_fee_rates(800_000_000_001), (95, 95));
        assert_eq!(get_fee_rates(4_000_000_000_000), (95, 95));
        assert_eq!(get_fee_rates(4_000_000_000_001), (90, 90));
    }

    #[test]
    fn splits_protocol_fee_for_referral() {
        let fees = calculate_fees(1, 1_000_000, true).unwrap();
        assert_eq!(fees.creator_fee, 10_000);
        assert_eq!(fees.protocol_fee, 7_000);
        assert_eq!(fees.referral_fee, 3_000);
        assert_eq!(fees.total_fee, 20_000);
    }

    #[test]
    fn calculates_referral_fees_at_different_tiers() {
        let amount = 10_000_000;

        let fees = calculate_fees(10_000_000_000_000, amount, true).unwrap();
        assert_eq!(fees.creator_fee, 90_000);
        assert_eq!(fees.protocol_fee, 63_000);
        assert_eq!(fees.referral_fee, 27_000);
        assert_eq!(fees.total_fee, 180_000);

        let fees = calculate_fees(200_000_000_000_000, amount, true).unwrap();
        assert_eq!(fees.creator_fee, 65_000);
        assert_eq!(fees.protocol_fee, 52_000);
        assert_eq!(fees.referral_fee, 13_000);
        assert_eq!(fees.total_fee, 130_000);

        let fees = calculate_fees(500_000_000_000_000, amount, true).unwrap();
        assert_eq!(fees.creator_fee, 50_000);
        assert_eq!(fees.protocol_fee, 42_500);
        assert_eq!(fees.referral_fee, 7_500);
        assert_eq!(fees.total_fee, 100_000);
    }

    #[test]
    fn exposes_referral_fee_tiers() {
        assert_eq!(get_referral_fee_rate(0), 3000);
        assert_eq!(get_referral_fee_rate(12_000_000_000_000), 3000);
        assert_eq!(get_referral_fee_rate(12_000_000_000_001), 2000);
        assert_eq!(get_referral_fee_rate(400_000_000_000_001), 1500);
    }

    #[test]
    fn uses_default_fee_rates_above_last_tier() {
        assert_eq!(get_fee_rates(400_000_000_000_001), (50, 50));
        assert_eq!(get_fee_rates(u64::MAX), (50, 50));
    }

    #[test]
    fn keeps_fee_tiers_non_zero_and_within_denominator() {
        for (market_cap, creator_fee_bps, protocol_fee_bps) in FEE_TIERS {
            assert!(*market_cap > 0);
            assert!(*creator_fee_bps + *protocol_fee_bps > 0);
            assert!(*creator_fee_bps + *protocol_fee_bps <= FEE_DENOMINATOR);
        }
    }

    #[test]
    fn keeps_fee_tiers_ordered_and_non_increasing() {
        for window in FEE_TIERS.windows(2) {
            let prev = window[0];
            let next = window[1];
            assert!(next.0 > prev.0);
            assert!(next.1 <= prev.1);
            assert!(next.2 <= prev.2);
            assert!(prev.1 + prev.2 <= FEE_DENOMINATOR);
            assert!(next.1 + next.2 <= FEE_DENOMINATOR);
        }
    }

    #[test]
    fn keeps_referral_fee_tiers_ordered_and_non_increasing() {
        for window in REFERRAL_FEE_TIERS.windows(2) {
            let prev = window[0];
            let next = window[1];
            assert!(next.0 > prev.0);
            assert!(next.1 <= prev.1);
            assert!(prev.1 <= FEE_DENOMINATOR / 2);
            assert!(next.1 <= FEE_DENOMINATOR / 2);
        }
        let default_referral_fee_rate = get_referral_fee_rate(u64::MAX);
        assert!(default_referral_fee_rate <= FEE_DENOMINATOR / 2);
    }

    #[test]
    fn keeps_referral_fee_tiers_non_zero_and_within_denominator() {
        for (market_cap, referral_fee_bps) in REFERRAL_FEE_TIERS {
            assert!(*market_cap > 0);
            assert!(*referral_fee_bps > 0);
            assert!(*referral_fee_bps <= FEE_DENOMINATOR);
        }
    }

    #[test]
    fn protocol_and_referral_match_base_protocol_fee() {
        let market_caps = [
            1,
            10_000_000_000_000,
            200_000_000_000_000,
            500_000_000_000_000,
        ];

        for market_cap in market_caps {
            let with_referral = calculate_fees(market_cap, 1_000_000, true).unwrap();
            let without_referral = calculate_fees(market_cap, 1_000_000, false).unwrap();

            assert_eq!(
                with_referral.protocol_fee + with_referral.referral_fee,
                without_referral.protocol_fee
            );
        }
    }

    #[test]
    fn handles_very_small_amounts() {
        let without_referral = calculate_fees(1_000_000_000_000, 1, false).unwrap();
        assert!(without_referral.total_fee <= 1);
        assert_eq!(
            without_referral.creator_fee + without_referral.protocol_fee,
            without_referral.total_fee
        );

        let with_referral = calculate_fees(1_000_000_000_000, 1, true).unwrap();
        assert!(with_referral.total_fee <= 1);
        assert_eq!(
            with_referral.protocol_fee + with_referral.referral_fee,
            without_referral.protocol_fee
        );
    }

    #[test]
    fn computes_market_cap_for_edge_reserves() {
        let mut small_snapshot = snapshot();
        small_snapshot.virtual_base_reserve = 1;
        small_snapshot.virtual_quote_reserve = 1;
        assert!(calculate_market_cap(&small_snapshot).unwrap() > 0);

        let mut equal_snapshot = snapshot();
        equal_snapshot.virtual_base_reserve = 1_000_000;
        equal_snapshot.virtual_quote_reserve = 1_000_000;
        assert_eq!(calculate_market_cap(&equal_snapshot).unwrap(), BASE_SUPPLY);

        let mut large_quote_snapshot = snapshot();
        large_quote_snapshot.virtual_quote_reserve = 1_000_000_000_000_000;
        assert!(calculate_market_cap(&large_quote_snapshot).unwrap() > 0);
    }

    #[test]
    fn initial_market_cap_stays_in_first_fee_tier() {
        let initial_market_cap = calculate_market_cap(&snapshot()).unwrap();
        assert!(u128::from(initial_market_cap) < 10_000_000_000_000u128);
        assert_eq!(
            get_fee_rates(initial_market_cap),
            (FEE_TIERS[0].1, FEE_TIERS[0].2)
        );
    }

    #[test]
    fn exposes_referral_rates_within_each_tier() {
        assert_eq!(get_referral_fee_rate(1_000_000_000_000), 3000);
        assert_eq!(get_referral_fee_rate(6_000_000_000_000), 3000);
        assert_eq!(get_referral_fee_rate(10_000_000_000_000), 3000);

        assert_eq!(get_referral_fee_rate(15_000_000_000_000), 2000);
        assert_eq!(get_referral_fee_rate(100_000_000_000_000), 2000);
        assert_eq!(get_referral_fee_rate(300_000_000_000_000), 2000);

        assert_eq!(
            get_referral_fee_rate(500_000_000_000_000),
            DEFAULT_REFERRAL_FEE
        );
        assert_eq!(
            get_referral_fee_rate(1_000_000_000_000_000),
            DEFAULT_REFERRAL_FEE
        );
    }
}

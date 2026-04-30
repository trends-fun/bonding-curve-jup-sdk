use anyhow::{ensure, Result};
use jupiter_amm_interface::{
    single_program_amm, try_get_account_data_and_owner, AccountMap, Amm, AmmContext, KeyedAccount,
    Quote, QuoteParams, SingleProgramAmm, Swap, SwapAndAccountMetas, SwapMode, SwapParams,
};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::{
    build_swap_account_metas, quote_for_mints, supports_mints, PoolSnapshot,
    SwapAccountMetasParams, BONDING_CURVE_LABEL, BONDING_CURVE_PROGRAM_ID,
    BONDING_CURVE_SWAP_ACCOUNTS_LEN, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuoteReferralPolicy {
    NoReferralContextInQuoteParams,
}

#[derive(Clone)]
pub struct BondingCurveAmm {
    key: Pubkey,
    state: PoolSnapshot,
}

single_program_amm!(
    BondingCurveAmm,
    BONDING_CURVE_PROGRAM_ID,
    BONDING_CURVE_LABEL
);

impl Amm for BondingCurveAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, _amm_context: &AmmContext) -> Result<Self> {
        ensure!(
            keyed_account.account.owner == BONDING_CURVE_PROGRAM_ID,
            "Unexpected owner for bonding curve pool"
        );

        Ok(Self {
            key: keyed_account.key,
            state: PoolSnapshot::try_from_account_data(&keyed_account.account.data)?,
        })
    }

    fn label(&self) -> String {
        BONDING_CURVE_LABEL.to_string()
    }

    fn program_id(&self) -> Pubkey {
        BONDING_CURVE_PROGRAM_ID
    }

    fn key(&self) -> Pubkey {
        self.key
    }

    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![self.state.base_mint, WSOL_MINT]
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![self.key]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let (pool_account_data, owner) = try_get_account_data_and_owner(account_map, &self.key)?;
        ensure!(
            *owner == BONDING_CURVE_PROGRAM_ID,
            "Unexpected owner for bonding curve pool"
        );
        self.state = PoolSnapshot::try_from_account_data(pool_account_data)?;
        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        ensure!(
            quote_params.swap_mode == SwapMode::ExactIn,
            "Bonding curve AMM only supports exact-in quotes"
        );

        self.ensure_supported_pair(quote_params.input_mint, quote_params.output_mint, "AMM")?;

        let sdk_quote = quote_for_mints(
            &self.state,
            quote_params.input_mint,
            quote_params.output_mint,
            quote_params.amount,
            self.quote_has_referral(),
        )?;

        Ok(to_jupiter_quote(quote_params.amount, sdk_quote))
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        ensure!(
            swap_params.swap_mode == SwapMode::ExactIn,
            "Bonding curve AMM only supports exact-in swaps"
        );

        self.ensure_supported_pair(
            swap_params.source_mint,
            swap_params.destination_mint,
            "swap",
        )?;

        Ok(SwapAndAccountMetas {
            swap: Swap::MeteoraDynamicBondingCurveSwapWithRemainingAccounts,
            account_metas: build_swap_account_metas(
                &self.state,
                SwapAccountMetasParams {
                    pool: self.key,
                    source_token_account: swap_params.source_token_account,
                    destination_token_account: swap_params.destination_token_account,
                    token_transfer_authority: swap_params.token_transfer_authority,
                    referral_token_account: referral_token_account(swap_params),
                    referral_placeholder: swap_params.placeholder_account_meta(),
                },
            ),
        })
    }

    fn get_accounts_len(&self) -> usize {
        BONDING_CURVE_SWAP_ACCOUNTS_LEN
    }

    fn supports_exact_out(&self) -> bool {
        false
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }

    fn program_dependencies(&self) -> Vec<(Pubkey, String)> {
        vec![
            (BONDING_CURVE_PROGRAM_ID, "bonding_curve".to_string()),
            (TOKEN_2022_PROGRAM_ID, "spl_token_2022".to_string()),
            (TOKEN_PROGRAM_ID, "spl_token".to_string()),
        ]
    }

    fn is_active(&self) -> bool {
        self.state.is_tradeable()
    }
}

impl BondingCurveAmm {
    fn ensure_supported_pair(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        context: &str,
    ) -> Result<()> {
        ensure!(
            supports_mints(&self.state, input_mint, output_mint),
            "Unsupported mint pair for bonding curve {context}"
        );
        Ok(())
    }

    fn quote_has_referral(&self) -> bool {
        match QuoteReferralPolicy::NoReferralContextInQuoteParams {
            QuoteReferralPolicy::NoReferralContextInQuoteParams => false,
        }
    }
}

fn referral_token_account(swap_params: &SwapParams) -> Option<Pubkey> {
    swap_params
        .quote_mint_to_referrer
        .and_then(|quote_mint_to_referrer| quote_mint_to_referrer.get(&WSOL_MINT))
        .copied()
}

fn to_jupiter_quote(in_amount: u64, sdk_quote: crate::QuoteResult) -> Quote {
    let creator_fee_bps = sdk_quote.fee_breakdown.creator_fee_bps;
    let protocol_fee_bps = sdk_quote.fee_breakdown.protocol_fee_bps;

    let bps = creator_fee_bps.saturating_add(protocol_fee_bps);
    let fee_pct = Decimal::from(bps) / Decimal::from(crate::get_fee_denominator());

    Quote {
        in_amount,
        out_amount: sdk_quote.amount_out,
        fee_amount: sdk_quote.fee_amount,
        fee_mint: sdk_quote.fee_mint,
        fee_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use jupiter_amm_interface::{FeeMode, QuoteMintToReferrer};
    use solana_account::Account;
    use solana_pubkey::pubkey;

    fn sample_snapshot() -> PoolSnapshot {
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

    fn mainnet_fixture_data() -> Vec<u8> {
        STANDARD
            .decode(include_str!("../tests/fixtures/mainnet_pool_8r9aukf8.b64").trim())
            .expect("mainnet fixture should decode")
    }

    fn mainnet_pool_key() -> Pubkey {
        pubkey!("8r9aukF8nPpk33R7eTZW7nkVuLq2jrENVyvKmoSNoHfU")
    }

    fn keyed_account(data: Vec<u8>) -> KeyedAccount {
        KeyedAccount {
            key: mainnet_pool_key(),
            account: Account {
                data,
                owner: BONDING_CURVE_PROGRAM_ID,
                ..Account::default()
            },
            params: None,
        }
    }

    #[test]
    fn adapter_reports_pool_key_for_updates() {
        let amm = BondingCurveAmm {
            key: Pubkey::new_unique(),
            state: sample_snapshot(),
        };

        assert_eq!(amm.get_accounts_to_update(), vec![amm.key]);
    }

    #[test]
    fn adapter_reports_expected_program_dependencies() {
        let amm = BondingCurveAmm {
            key: Pubkey::new_unique(),
            state: sample_snapshot(),
        };

        assert_eq!(
            amm.program_dependencies(),
            vec![
                (BONDING_CURVE_PROGRAM_ID, "bonding_curve".to_string()),
                (TOKEN_2022_PROGRAM_ID, "spl_token_2022".to_string()),
                (TOKEN_PROGRAM_ID, "spl_token".to_string()),
            ]
        );
    }

    #[test]
    fn adapter_quotes_real_mainnet_fixture() {
        let keyed_account = keyed_account(mainnet_fixture_data());
        let amm = BondingCurveAmm::from_keyed_account(&keyed_account, &AmmContext::default())
            .expect("fixture account should build adapter");

        let quote = amm
            .quote(&QuoteParams {
                amount: 1_000_000,
                input_mint: WSOL_MINT,
                output_mint: pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
                swap_mode: SwapMode::ExactIn,
                fee_mode: FeeMode::Normal,
            })
            .expect("fixture quote should succeed");

        assert_eq!(quote.out_amount, 48_997_599_117);
        assert_eq!(quote.fee_amount, 20_000);
        assert_eq!(quote.fee_mint, WSOL_MINT);
        assert_eq!(
            amm.get_reserve_mints().as_slice(),
            &[
                pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
                WSOL_MINT,
            ]
        );
    }

    #[test]
    fn adapter_builds_swap_with_referral_for_real_fixture() {
        let keyed_account = keyed_account(mainnet_fixture_data());
        let amm = BondingCurveAmm::from_keyed_account(&keyed_account, &AmmContext::default())
            .expect("fixture account should build adapter");
        let source_token_account = Pubkey::new_unique();
        let destination_token_account = Pubkey::new_unique();
        let token_transfer_authority = Pubkey::new_unique();
        let jupiter_program_id = Pubkey::new_unique();
        let referral_token_account = Pubkey::new_unique();
        let mut quote_mint_to_referrer = QuoteMintToReferrer::default();
        quote_mint_to_referrer.insert(WSOL_MINT, referral_token_account);

        let swap = amm
            .get_swap_and_account_metas(&SwapParams {
                swap_mode: SwapMode::ExactIn,
                in_amount: 1_000_000,
                out_amount: 48_997_599_117,
                source_mint: WSOL_MINT,
                destination_mint: pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
                source_token_account,
                destination_token_account,
                token_transfer_authority,
                user: Pubkey::new_unique(),
                payer: Pubkey::new_unique(),
                quote_mint_to_referrer: Some(&quote_mint_to_referrer),
                jupiter_program_id: &jupiter_program_id,
                missing_dynamic_accounts_as_default: false,
            })
            .expect("swap metas should build");

        assert_eq!(
            swap.swap,
            Swap::MeteoraDynamicBondingCurveSwapWithRemainingAccounts
        );
        assert_eq!(swap.account_metas.len(), BONDING_CURVE_SWAP_ACCOUNTS_LEN);
        assert_eq!(
            swap.account_metas[2],
            solana_instruction::AccountMeta::new(mainnet_pool_key(), false)
        );
        assert_eq!(
            swap.account_metas[4],
            solana_instruction::AccountMeta::new(source_token_account, false)
        );
        assert_eq!(
            swap.account_metas[5],
            solana_instruction::AccountMeta::new(destination_token_account, false)
        );
        assert_eq!(
            swap.account_metas[10],
            solana_instruction::AccountMeta::new_readonly(token_transfer_authority, true)
        );
        assert_eq!(
            swap.account_metas[13],
            solana_instruction::AccountMeta::new(referral_token_account, false)
        );
    }

    #[test]
    fn adapter_rejects_exact_out_swap_metas() {
        let keyed_account = keyed_account(mainnet_fixture_data());
        let amm = BondingCurveAmm::from_keyed_account(&keyed_account, &AmmContext::default())
            .expect("fixture account should build adapter");

        let swap_result = amm.get_swap_and_account_metas(&SwapParams {
            swap_mode: SwapMode::ExactOut,
            in_amount: 1_000_000,
            out_amount: 48_997_599_117,
            source_mint: WSOL_MINT,
            destination_mint: pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
            source_token_account: Pubkey::new_unique(),
            destination_token_account: Pubkey::new_unique(),
            token_transfer_authority: Pubkey::new_unique(),
            user: Pubkey::new_unique(),
            payer: Pubkey::new_unique(),
            quote_mint_to_referrer: None,
            jupiter_program_id: &Pubkey::new_unique(),
            missing_dynamic_accounts_as_default: false,
        });

        let err = match swap_result {
            Ok(_) => panic!("exact-out swaps should be rejected"),
            Err(err) => err,
        };

        assert!(err
            .to_string()
            .contains("Bonding curve AMM only supports exact-in swaps"));
    }

    #[test]
    fn adapter_update_reloads_pool_state_from_account_map() {
        let keyed_account = keyed_account(mainnet_fixture_data());
        let mut amm = BondingCurveAmm::from_keyed_account(&keyed_account, &AmmContext::default())
            .expect("fixture account should build adapter");

        let before = amm
            .quote(&QuoteParams {
                amount: 1_000_000,
                input_mint: WSOL_MINT,
                output_mint: pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
                swap_mode: SwapMode::ExactIn,
                fee_mode: FeeMode::Normal,
            })
            .expect("initial quote should succeed");

        let mut updated_data = mainnet_fixture_data();
        updated_data[160..168].copy_from_slice(&40_000_000_000u64.to_le_bytes());

        let mut account_map = AccountMap::default();
        account_map.insert(
            mainnet_pool_key(),
            Account {
                data: updated_data,
                owner: BONDING_CURVE_PROGRAM_ID,
                ..Account::default()
            },
        );

        amm.update(&account_map)
            .expect("update should reload state");

        let after = amm
            .quote(&QuoteParams {
                amount: 1_000_000,
                input_mint: WSOL_MINT,
                output_mint: pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND"),
                swap_mode: SwapMode::ExactIn,
                fee_mode: FeeMode::Normal,
            })
            .expect("updated quote should succeed");

        assert_eq!(before.out_amount, 48_997_599_117);
        assert_eq!(after.out_amount, 24_499_399_764);
        assert!(after.out_amount < before.out_amount);
    }

    #[test]
    fn adapter_update_rejects_unexpected_owner() {
        let keyed_account = keyed_account(mainnet_fixture_data());
        let mut amm = BondingCurveAmm::from_keyed_account(&keyed_account, &AmmContext::default())
            .expect("fixture account should build adapter");

        let mut account_map = AccountMap::default();
        account_map.insert(
            mainnet_pool_key(),
            Account {
                data: mainnet_fixture_data(),
                owner: Pubkey::new_unique(),
                ..Account::default()
            },
        );

        let err = amm
            .update(&account_map)
            .expect_err("update should reject non-program owner");
        assert!(err
            .to_string()
            .contains("Unexpected owner for bonding curve pool"));
    }

    #[test]
    fn adapter_marks_completed_pool_inactive() {
        let mut amm = BondingCurveAmm {
            key: Pubkey::new_unique(),
            state: sample_snapshot(),
        };
        amm.state.quote_reserve = crate::MIGRATION_QUOTE_THRESHOLD;

        assert!(!amm.is_active());
    }

    #[test]
    fn adapter_marks_migrated_pool_inactive() {
        let mut amm = BondingCurveAmm {
            key: Pubkey::new_unique(),
            state: sample_snapshot(),
        };
        amm.state.is_migrated = 1;

        assert!(!amm.is_active());
    }

    #[test]
    fn to_jupiter_quote_calculates_fee_pct_from_bps() {
        let sdk_quote = crate::QuoteResult {
            amount_in: 1_000_000,
            amount_out: 500_000,
            fee_amount: 5_000,
            fee_mint: crate::WSOL_MINT,
            fee_breakdown: crate::FeeBreakdown {
                creator_fee: 2_500,
                protocol_fee: 2_500,
                referral_fee: 0,
                total_fee: 5_000,
                creator_fee_bps: 50,
                protocol_fee_bps: 50,
            },
            market_cap: 100_000_000_000_000,
        };

        // 50 bps + 50 bps = 100 bps = 0.0100
        let quote = to_jupiter_quote(1_000_000, sdk_quote);
        assert_eq!(
            quote.fee_pct,
            rust_decimal::Decimal::from(100)
                / rust_decimal::Decimal::from(crate::get_fee_denominator())
        );
    }

    #[test]
    fn to_jupiter_quote_avoids_division_by_amount() {
        // Provide extreme token to WSOL ratio where fee_amount is tiny but in_amount is huge.
        // If it were using `fee_amount / in_amount`, fee_pct would be completely incorrect.
        let in_amount = 1_000_000_000;
        let sdk_quote = crate::QuoteResult {
            amount_in: in_amount,
            amount_out: 10,
            fee_amount: 1, // 1 WSOL lamport fee
            fee_mint: crate::WSOL_MINT,
            fee_breakdown: crate::FeeBreakdown {
                creator_fee: 1,
                protocol_fee: 0,
                referral_fee: 0,
                total_fee: 1,
                creator_fee_bps: 100,
                protocol_fee_bps: 100, // 200 bps total (2%)
            },
            market_cap: 1_000_000, // Not relevant here
        };

        let quote = to_jupiter_quote(in_amount, sdk_quote);

        // Ensure it uses the BPS strictly instead of 1 / 1_000_000_000
        assert_eq!(
            quote.fee_pct,
            rust_decimal::Decimal::from(200)
                / rust_decimal::Decimal::from(crate::get_fee_denominator())
        );
    }
}

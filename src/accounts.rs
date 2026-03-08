use crate::{PoolSnapshot, WSOL_MINT};
use solana_instruction::AccountMeta;
use solana_pubkey::{pubkey, Pubkey};

pub const BONDING_CURVE_LABEL: &str = "Trends";
pub const BONDING_CURVE_PROGRAM_ID: Pubkey =
    pubkey!("CURVEmPpijXDTNdqrA9PGP1io2rkgiVXH26xdXVGLLfz");
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const BONDING_CURVE_SWAP_ACCOUNTS_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwapAccountMetasParams {
    pub pool: Pubkey,
    pub source_token_account: Pubkey,
    pub destination_token_account: Pubkey,
    pub token_transfer_authority: Pubkey,
    pub referral_token_account: Option<Pubkey>,
    pub referral_placeholder: AccountMeta,
}

pub fn build_swap_account_metas(
    snapshot: &PoolSnapshot,
    params: SwapAccountMetasParams,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(BONDING_CURVE_PROGRAM_ID, false),
        AccountMeta::new_readonly(config_address(), false),
        AccountMeta::new(params.pool, false),
        AccountMeta::new_readonly(pool_authority(), false),
        AccountMeta::new(params.source_token_account, false),
        AccountMeta::new(params.destination_token_account, false),
        AccountMeta::new_readonly(snapshot.base_mint, false),
        AccountMeta::new_readonly(WSOL_MINT, false),
        AccountMeta::new(snapshot.base_vault, false),
        AccountMeta::new(snapshot.quote_vault, false),
        AccountMeta::new_readonly(params.token_transfer_authority, true),
        AccountMeta::new_readonly(TOKEN_2022_PROGRAM_ID, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        referral_account_meta(params.referral_token_account, params.referral_placeholder),
        AccountMeta::new_readonly(event_authority(), false),
        AccountMeta::new_readonly(BONDING_CURVE_PROGRAM_ID, false),
    ]
}

pub fn config_address() -> Pubkey {
    Pubkey::find_program_address(&[b"config"], &BONDING_CURVE_PROGRAM_ID).0
}

pub fn pool_authority() -> Pubkey {
    Pubkey::find_program_address(&[b"pool_authority"], &BONDING_CURVE_PROGRAM_ID).0
}

pub fn event_authority() -> Pubkey {
    Pubkey::find_program_address(&[b"__event_authority"], &BONDING_CURVE_PROGRAM_ID).0
}

pub fn referral_account_meta(
    referral_token_account: Option<Pubkey>,
    referral_placeholder: AccountMeta,
) -> AccountMeta {
    referral_token_account
        .map(|account| AccountMeta::new(account, false))
        .unwrap_or(referral_placeholder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> PoolSnapshot {
        PoolSnapshot {
            base_mint: Pubkey::new_unique(),
            base_vault: Pubkey::new_unique(),
            quote_vault: Pubkey::new_unique(),
            base_reserve: 1_000_000_000_000_000,
            quote_reserve: 10_000_000_000,
            virtual_base_reserve: 1_000_000_000_000_000,
            virtual_quote_reserve: 20_000_000_000,
        }
    }

    #[test]
    fn builds_swap_account_metas_in_contract_order() {
        let snapshot = sample_snapshot();
        let pool = Pubkey::new_unique();
        let source_token_account = Pubkey::new_unique();
        let destination_token_account = Pubkey::new_unique();
        let token_transfer_authority = Pubkey::new_unique();
        let referral_placeholder = AccountMeta::new_readonly(Pubkey::new_unique(), false);
        let params = SwapAccountMetasParams {
            pool,
            source_token_account,
            destination_token_account,
            token_transfer_authority,
            referral_token_account: None,
            referral_placeholder: referral_placeholder.clone(),
        };

        let metas = build_swap_account_metas(&snapshot, params);

        assert_eq!(metas.len(), BONDING_CURVE_SWAP_ACCOUNTS_LEN);
        assert_eq!(
            metas[0],
            AccountMeta::new_readonly(BONDING_CURVE_PROGRAM_ID, false)
        );
        assert_eq!(metas[1], AccountMeta::new_readonly(config_address(), false));
        assert_eq!(metas[3], AccountMeta::new_readonly(pool_authority(), false));
        assert_eq!(metas[2], AccountMeta::new(pool, false));
        assert_eq!(metas[4], AccountMeta::new(source_token_account, false));
        assert_eq!(metas[5], AccountMeta::new(destination_token_account, false));
        assert_eq!(
            metas[6],
            AccountMeta::new_readonly(snapshot.base_mint, false)
        );
        assert_eq!(metas[7], AccountMeta::new_readonly(WSOL_MINT, false));
        assert_eq!(
            metas[11],
            AccountMeta::new_readonly(TOKEN_2022_PROGRAM_ID, false)
        );
        assert_eq!(
            metas[12],
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false)
        );
        assert_eq!(
            metas[10],
            AccountMeta::new_readonly(token_transfer_authority, true)
        );
        assert_eq!(metas[13], referral_placeholder);
        assert_eq!(
            metas[14],
            AccountMeta::new_readonly(event_authority(), false)
        );
        assert_eq!(
            metas[15],
            AccountMeta::new_readonly(BONDING_CURVE_PROGRAM_ID, false)
        );
    }

    #[test]
    fn derives_pool_authority_from_program_id() {
        let (derived, _) =
            Pubkey::find_program_address(&[b"pool_authority"], &BONDING_CURVE_PROGRAM_ID);
        assert_eq!(pool_authority(), derived);
    }

    #[test]
    fn uses_writable_referral_account_when_present() {
        let referral_token_account = Pubkey::new_unique();
        let meta = referral_account_meta(
            Some(referral_token_account),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
        );

        assert_eq!(meta, AccountMeta::new(referral_token_account, false));
    }
}

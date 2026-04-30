use solana_pubkey::{pubkey, Pubkey};

use crate::QuoteError;

pub const BONDING_CURVE_POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];
#[cfg(feature = "devnet")]
pub const MIGRATION_QUOTE_THRESHOLD: u64 = 10_000_000_000;
#[cfg(not(feature = "devnet"))]
pub const MIGRATION_QUOTE_THRESHOLD: u64 = 85_000_000_000;
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolSnapshot {
    pub base_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub base_reserve: u64,
    pub quote_reserve: u64,
    pub virtual_base_reserve: u64,
    pub virtual_quote_reserve: u64,
    pub is_migrated: u8,
}

impl PoolSnapshot {
    pub fn try_from_account_data(data: &[u8]) -> Result<Self, QuoteError> {
        const PUBKEY_LEN: usize = 32;
        const U64_LEN: usize = 8;
        const U8_LEN: usize = 1;
        const ACCOUNT_LEN: usize = 8 + PUBKEY_LEN * 4 + U64_LEN * 6 + U8_LEN + 7 + U64_LEN * 15;

        if data.len() < ACCOUNT_LEN {
            return Err(QuoteError::PoolAccountTooSmall);
        }
        if data[..8] != BONDING_CURVE_POOL_DISCRIMINATOR {
            return Err(QuoteError::UnexpectedPoolDiscriminator);
        }

        let body = &data[8..];
        Ok(Self {
            base_mint: read_pubkey(body, 32)?,
            base_vault: read_pubkey(body, 64)?,
            quote_vault: read_pubkey(body, 96)?,
            base_reserve: read_u64(body, 128)?,
            quote_reserve: read_u64(body, 136)?,
            virtual_base_reserve: read_u64(body, 144)?,
            virtual_quote_reserve: read_u64(body, 152)?,
            is_migrated: read_u8(body, 176)?,
        })
    }

    pub fn is_completed(&self) -> bool {
        self.quote_reserve >= MIGRATION_QUOTE_THRESHOLD
    }

    pub fn is_tradeable(&self) -> bool {
        self.is_migrated == 0
            && !self.is_completed()
            && self.virtual_base_reserve > 0
            && self.virtual_quote_reserve > 0
    }
}

fn read_pubkey(data: &[u8], offset: usize) -> Result<Pubkey, QuoteError> {
    let bytes = data
        .get(offset..offset + 32)
        .ok_or(QuoteError::MissingPubkeyBytes(offset))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| QuoteError::IntegerConversionOverflow)?;
    Ok(Pubkey::new_from_array(array))
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64, QuoteError> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(QuoteError::MissingU64Bytes(offset))?;
    let array: [u8; 8] = bytes
        .try_into()
        .map_err(|_| QuoteError::IntegerConversionOverflow)?;
    Ok(u64::from_le_bytes(array))
}

fn read_u8(data: &[u8], offset: usize) -> Result<u8, QuoteError> {
    data.get(offset)
        .copied()
        .ok_or(QuoteError::MissingU8Byte(offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use solana_pubkey::pubkey;

    fn mainnet_fixture_data() -> Vec<u8> {
        STANDARD
            .decode(include_str!("../tests/fixtures/mainnet_pool_8r9aukf8.b64").trim())
            .expect("mainnet fixture should decode")
    }

    #[test]
    fn parses_pool_snapshot_from_account_data() {
        let creator = Pubkey::new_unique();
        let base_mint = Pubkey::new_unique();
        let base_vault = Pubkey::new_unique();
        let quote_vault = Pubkey::new_unique();

        let mut data = Vec::new();
        data.extend_from_slice(&BONDING_CURVE_POOL_DISCRIMINATOR);
        data.extend_from_slice(creator.as_ref());
        data.extend_from_slice(base_mint.as_ref());
        data.extend_from_slice(base_vault.as_ref());
        data.extend_from_slice(quote_vault.as_ref());
        data.extend_from_slice(&123u64.to_le_bytes());
        data.extend_from_slice(&456u64.to_le_bytes());
        data.extend_from_slice(&789u64.to_le_bytes());
        data.extend_from_slice(&999u64.to_le_bytes());
        data.extend_from_slice(&1u64.to_le_bytes());
        data.extend_from_slice(&2u64.to_le_bytes());
        data.push(1);
        data.extend_from_slice(&[0u8; 7]);
        data.extend_from_slice(&[0u8; 15 * 8]);

        let snapshot = PoolSnapshot::try_from_account_data(&data).unwrap();
        assert_eq!(snapshot.base_mint, base_mint);
        assert_eq!(snapshot.base_vault, base_vault);
        assert_eq!(snapshot.quote_vault, quote_vault);
        assert_eq!(snapshot.base_reserve, 123);
        assert_eq!(snapshot.quote_reserve, 456);
        assert_eq!(snapshot.virtual_base_reserve, 789);
        assert_eq!(snapshot.virtual_quote_reserve, 999);
        assert_eq!(snapshot.is_migrated, 1);
    }

    #[test]
    fn rejects_short_account_data() {
        let err = PoolSnapshot::try_from_account_data(&[]).unwrap_err();
        assert_eq!(err, QuoteError::PoolAccountTooSmall);
    }

    #[test]
    fn rejects_wrong_discriminator() {
        let mut data = vec![0u8; 8 + 32 * 4 + 8 * 22];
        data[..8].copy_from_slice(&[1; 8]);

        let err = PoolSnapshot::try_from_account_data(&data).unwrap_err();
        assert_eq!(err, QuoteError::UnexpectedPoolDiscriminator);
    }

    #[test]
    fn parses_expected_mainnet_pool_snapshot() {
        let snapshot = PoolSnapshot::try_from_account_data(&mainnet_fixture_data()).unwrap();

        assert_eq!(
            snapshot.base_mint,
            pubkey!("CMNKDgGkQmVRr8RXV3gCrceGdCmm5w4ZBLgA6SdvTRND")
        );
        assert_eq!(
            snapshot.base_vault,
            pubkey!("BKCjUvubFHFydBPH9AUNFMRfmQa1gSgV3HHE8Gk92EvV")
        );
        assert_eq!(
            snapshot.quote_vault,
            pubkey!("AzqHNkVvsRjX5vD2NpKHtXq9XEJWET1V7HDQTzDPJq4N")
        );
        assert_eq!(snapshot.base_reserve, 1_000_000_000_000_000);
        assert_eq!(snapshot.quote_reserve, 0);
        assert_eq!(snapshot.virtual_base_reserve, 1_000_000_000_000_000);
        assert_eq!(snapshot.virtual_quote_reserve, 20_000_000_000);
        assert_eq!(snapshot.is_migrated, 0);
    }

    #[test]
    fn detects_completed_pool_from_quote_reserve() {
        let snapshot = PoolSnapshot {
            base_mint: Pubkey::new_unique(),
            base_vault: Pubkey::new_unique(),
            quote_vault: Pubkey::new_unique(),
            base_reserve: 1_000_000_000_000_000,
            quote_reserve: MIGRATION_QUOTE_THRESHOLD,
            virtual_base_reserve: 1_000_000_000_000_000,
            virtual_quote_reserve: 20_000_000_000,
            is_migrated: 0,
        };

        assert!(snapshot.is_completed());
        assert!(!snapshot.is_tradeable());
    }
}

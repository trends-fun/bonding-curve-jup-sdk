use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuoteError {
    #[error("Invalid zero amount")]
    InvalidZeroAmount,
    #[error("Unsupported mint pair")]
    UnsupportedPair,
    #[error("Pool account data is too small")]
    PoolAccountTooSmall,
    #[error("Unexpected pool discriminator")]
    UnexpectedPoolDiscriminator,
    #[error("Missing pubkey bytes at offset {0}")]
    MissingPubkeyBytes(usize),
    #[error("Missing u64 bytes at offset {0}")]
    MissingU64Bytes(usize),
    #[error("Pool has no virtual liquidity")]
    ZeroLiquidity,
    #[error("Insufficient base reserve")]
    InsufficientBaseReserve,
    #[error("Insufficient quote reserve")]
    InsufficientQuoteReserve,
    #[error("Bonding curve math overflow")]
    MathOverflow,
    #[error("Bonding curve math underflow")]
    MathUnderflow,
    #[error("Bonding curve integer conversion overflow")]
    IntegerConversionOverflow,
    #[error("Missing u8 byte at offset {0}")]
    MissingU8Byte(usize),
    #[error("Pool has completed and can no longer trade on the curve")]
    PoolCompleted,
    #[error("Pool has migrated and can no longer trade on the curve")]
    PoolMigrated,
}

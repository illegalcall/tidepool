use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Deposit amounts must be greater than zero")]
    ZeroDeposit,

    #[msg("Insufficient shares for withdrawal")]
    InsufficientShares,

    #[msg("Vault has no active position to rebalance")]
    NoActivePosition,

    #[msg("Position is still in range — rebalance not needed")]
    RebalanceNotNeeded,

    #[msg("Only the vault keeper can perform this action")]
    UnauthorizedKeeper,

    #[msg("Only the vault authority can perform this action")]
    UnauthorizedAuthority,

    #[msg("Invalid fee configuration")]
    InvalidFeeConfig,

    #[msg("Invalid tick range for rebalance")]
    InvalidTickRange,

    #[msg("Arithmetic overflow")]
    MathOverflow,

    #[msg("Vault is paused")]
    VaultPaused,

    #[msg("No fees to compound")]
    NoFeesToCompound,

    #[msg("Vault total shares must be non-zero for withdrawal")]
    NoShares,
}

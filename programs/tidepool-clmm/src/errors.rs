use anchor_lang::prelude::*;

#[error_code]
pub enum TidePoolError {
    #[msg("Invalid tick spacing value")]
    InvalidTickSpacing,

    #[msg("Tick index out of supported range")]
    TickOutOfRange,

    #[msg("Tick index not aligned to tick spacing")]
    TickNotAligned,

    #[msg("Tick lower must be less than tick upper")]
    InvalidTickRange,

    #[msg("Sqrt price out of bounds")]
    SqrtPriceOutOfBounds,

    #[msg("Liquidity amount must be greater than zero")]
    ZeroLiquidity,

    #[msg("Insufficient token amount for desired liquidity")]
    InsufficientTokenAmount,

    #[msg("Token amount exceeds maximum specified")]
    TokenMaxExceeded,

    #[msg("Token amount below minimum specified")]
    TokenMinNotMet,

    #[msg("Swap amount must be greater than zero")]
    ZeroSwapAmount,

    #[msg("Sqrt price limit reached during swap")]
    SqrtPriceLimitReached,

    #[msg("No liquidity available in the current range")]
    NoLiquidity,

    #[msg("Overflow during arithmetic operation")]
    MathOverflow,

    #[msg("Division by zero")]
    DivisionByZero,

    #[msg("Invalid tick array for the current swap direction")]
    InvalidTickArray,

    #[msg("Tick array start index does not match expected")]
    TickArrayMismatch,

    #[msg("Position has no fees to collect")]
    NoFeesToCollect,

    #[msg("Unauthorized: only pool authority can perform this action")]
    Unauthorized,

    #[msg("Pool is currently paused")]
    PoolPaused,

    #[msg("Invalid fee rate — must be between 1 and 10000")]
    InvalidFeeRate,

    #[msg("Position not empty — remove liquidity before closing")]
    PositionNotEmpty,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
}

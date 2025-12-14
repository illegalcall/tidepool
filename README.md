# TidePool - Concentrated Liquidity AMM + Auto-Rebalancing Vault

A composable DeFi protocol on Solana consisting of two Anchor programs:

1. **TidePool CLMM** - Concentrated liquidity AMM with tick-based pricing (Uniswap V3 mechanics adapted for Solana)
2. **TideVault** - Auto-rebalancing vault that manages CLMM positions via CPI, auto-compounds fees, and distributes yield to depositors

## Architecture

```
  Program 1: CLMM                        Program 2: Vault
+----------------------------+          +---------------------------+
|                            |   CPI    |                           |
|  Pool State                |<---------|  Vault State              |
|    sqrt_price (Q64)        |          |    total_shares           |
|    tick_current            |--------->|    strategy_cfg           |
|    liquidity               |          |    last_rebalance         |
|    fee_growth_global       |          |                           |
|                            |          |  User Shares              |
|  Tick Arrays               |          |    deposited_a/b          |
|    64 ticks/array          |          |    share_amount           |
|                            |          |                           |
|  Positions (NFT)           |          |  Rebalance Engine         |
|    tick_lower/upper        |          |    keeper-trigger         |
|    liquidity               |          |                           |
|    fees_owed               |          +---------------------------+
+----------------------------+

  Off-Chain Services                      Frontend (Next.js)
+----------------------------+          +---------------------------+
| Rebalance Keeper (Rust)    |          | Pool LP Interface         |
| GraphQL Indexer            |          | Vault Deposit/Withdraw    |
| APY Calculator             |          | Liquidity Heatmap         |
+----------------------------+          +---------------------------+
```

## Core DeFi Math

### Q64.64 Fixed-Point Arithmetic
All price calculations use Q64.64 fixed-point representation (u128 where value = raw >> 64). This avoids floating-point entirely, which is critical for deterministic on-chain computation.

### Tick System
- Price space is divided into discrete ticks: `price = 1.0001^tick`
- `sqrt_price = 1.0001^(tick/2)` stored in Q64.64
- Tick arrays split across separate accounts (64 ticks per array) to respect Solana's 10KB account size limit
- Binary exponentiation with precomputed powers for `tick_to_sqrt_price`

### Concentrated Liquidity
- LPs provide liquidity within a chosen `[tick_lower, tick_upper]` range
- Only in-range liquidity earns fees, capital efficiency scales with range tightness
- Token amounts calculated via:
  - `delta_a = L * (1/sqrt_lower - 1/sqrt_upper)` (token A needed)
  - `delta_b = L * (sqrt_upper - sqrt_lower)` (token B needed)

### Fee Accumulation
Follows the `feeGrowthGlobal` / `feeGrowthOutside` pattern:
```
fee_growth_inside = fee_growth_global - fee_growth_below(lower) - fee_growth_above(upper)
fees_owed = liquidity * (fee_growth_inside - fee_growth_inside_last)
```

### Swap Execution
Multi-step swap across tick boundaries:
1. Find next initialized tick in swap direction
2. Compute swap step within current tick range (constant-product math)
3. If swap reaches next tick, cross it (update active liquidity from `liquidity_net`)
4. Accumulate fees per step, update `fee_growth_global`
5. Repeat until amount exhausted or price limit reached

## Vault Strategy

### ERC-4626 Style Share Accounting
- First deposit: `shares = sqrt(amount_a * amount_b)`
- Subsequent: `shares = min(amount_a/total_a, amount_b/total_b) * total_shares`
- Withdrawal: proportional `(shares/total_shares) * vault_value`

### Auto-Rebalancing
- Keeper monitors pool `tick_current` vs vault's active position range
- Triggers rebalance when price moves within `rebalance_threshold_bps` of range edge
- Calculates new optimal range centered on current tick, aligned to tick spacing
- Executes: remove old liquidity, collect fees, open new position, add liquidity

### Fee Compounding
- Keeper periodically collects earned trading fees from the CLMM position
- Deducts performance fee (configurable, max 30%)
- Reinvests remaining fees back into the position

## Project Structure

```
tidepool/
  programs/
    tidepool-clmm/             # Concentrated liquidity AMM
      src/
        lib.rs                 # Program entrypoint
        state/                 # Pool, TickArray, Position accounts
        instructions/          # initialize, add/remove liquidity, swap, collect fees
        math/                  # Q64.64, tick math, sqrt price, swap, fee calculations
        errors.rs              # Custom error codes
        events.rs              # On-chain events for indexing
    tidepool-vault/            # Auto-rebalancing vault
      src/
        lib.rs                 # Program entrypoint
        state/                 # Vault, UserReceipt accounts
        instructions/          # initialize, deposit, withdraw, rebalance, compound
        errors.rs
        events.rs
  keeper/                      # Off-chain Rust services
    src/
      main.rs                  # CLI entrypoint
      rebalancer.rs            # Rebalance keeper bot
      indexer.rs               # Account indexer + metrics
  tests/
    tidepool.ts                # Anchor integration tests
  Anchor.toml
  Cargo.toml
  README.md
```

## Building

```bash
# Prerequisites: Solana CLI, Anchor CLI, Rust, Node.js

# Build programs
anchor build

# Run tests
anchor test

# Run keeper (devnet)
cd keeper
cargo run -- --rpc-url https://api.devnet.solana.com --vault <VAULT_PUBKEY>

# Dry run mode
cargo run -- --rpc-url https://api.devnet.solana.com --vault <VAULT_PUBKEY> --dry-run
```

## Security Considerations

- All arithmetic uses `checked_*` operations, no unchecked overflow possible
- Q64.64 math uses custom U256 type for intermediate multiplication to prevent precision loss
- Oracle staleness checks on all price-dependent operations
- PDA seeds include relevant discriminators to prevent account substitution
- Position NFTs prevent unauthorized liquidity withdrawal
- Vault keeper role is separate from authority, keeper can rebalance but not withdraw funds
- Fee rates capped (max 30% performance, max 5% management)
- Emergency pause on both programs

## Key Design Decisions

1. **Tick arrays as separate accounts**: Solana's 10KB account limit makes storing all ticks in one account impossible. Arrays of 64 ticks per account, addressed by PDA with start_tick_index.

2. **Vault as separate program (not embedded)**: Enables composability, other vaults/strategies can CPI into the CLMM independently. Mirrors how Kamino operates on top of Orca Whirlpools.

3. **Q64.64 over floating point**: Deterministic computation critical for on-chain consensus. Custom U256 for intermediate products avoids overflow in `u128 * u128` operations.

4. **Keeper-driven rebalancing (not automated on-chain)**: On-chain automation via clockwork/cron is fragile. Off-chain keepers with Jito bundles provide more reliable execution with MEV protection.

## References

- [Uniswap V3 Whitepaper](https://uniswap.org/whitepaper-v3.pdf) - Core CLMM math
- [Orca Whirlpools](https://github.com/orca-so/whirlpools) - Solana CLMM reference implementation
- [Kamino Finance](https://kamino.finance/) - Vault-over-CLMM pattern
- [Sealevel Attacks](https://github.com/coral-xyz/sealevel-attacks) - Security patterns

## License

MIT

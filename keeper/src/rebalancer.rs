use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Vault state deserialized from on-chain account data.
/// Mirrors the Anchor account layout (skipping 8-byte discriminator).
#[derive(Debug)]
struct VaultState {
    pub authority: Pubkey,
    pub keeper: Pubkey,
    pub pool: Pubkey,
    pub active_tick_lower: i32,
    pub active_tick_upper: i32,
    pub has_active_position: bool,
    pub rebalance_threshold_bps: u16,
    pub tick_range_multiplier: u8,
    pub rebalance_count: u32,
    pub paused: bool,
}

/// Pool state deserialized from CLMM program.
#[derive(Debug)]
struct PoolState {
    pub sqrt_price: u128,
    pub tick_current_index: i32,
    pub tick_spacing: u16,
    pub liquidity: u128,
}

/// Determines if a rebalance is needed based on current pool tick vs vault range.
fn should_rebalance(pool: &PoolState, vault: &VaultState) -> bool {
    if !vault.has_active_position {
        return false;
    }
    if vault.paused {
        return false;
    }

    let current_tick = pool.tick_current_index;

    // Price moved outside active range entirely
    if current_tick < vault.active_tick_lower || current_tick >= vault.active_tick_upper {
        info!(
            "Price out of range: tick={} range=[{}, {}]",
            current_tick, vault.active_tick_lower, vault.active_tick_upper
        );
        return true;
    }

    // Price near edge of range (within threshold)
    let range_width = vault.active_tick_upper - vault.active_tick_lower;
    let threshold = range_width * vault.rebalance_threshold_bps as i32 / 10000;

    if current_tick < vault.active_tick_lower + threshold {
        info!(
            "Price near lower edge: tick={} threshold_tick={}",
            current_tick,
            vault.active_tick_lower + threshold
        );
        return true;
    }
    if current_tick > vault.active_tick_upper - threshold {
        info!(
            "Price near upper edge: tick={} threshold_tick={}",
            current_tick,
            vault.active_tick_upper - threshold
        );
        return true;
    }

    false
}

/// Calculate the optimal new tick range centered on the current price.
fn calculate_optimal_range(pool: &PoolState, vault: &VaultState) -> (i32, i32) {
    let current_tick = pool.tick_current_index;
    let tick_spacing = pool.tick_spacing as i32;
    let range_width = vault.tick_range_multiplier as i32 * tick_spacing;

    // Center around current tick, aligned to tick spacing
    let tick_lower = ((current_tick - range_width) / tick_spacing) * tick_spacing;
    let tick_upper = ((current_tick + range_width) / tick_spacing) * tick_spacing;

    (tick_lower, tick_upper)
}

/// Main rebalancer loop.
pub async fn run_rebalancer(
    rpc: Arc<RpcClient>,
    keeper: Keypair,
    vault_pubkey: Pubkey,
    poll_interval_ms: u64,
    dry_run: bool,
) -> Result<()> {
    let interval = Duration::from_millis(poll_interval_ms);

    loop {
        match check_and_rebalance(&rpc, &keeper, &vault_pubkey, dry_run).await {
            Ok(rebalanced) => {
                if rebalanced {
                    info!("Rebalance executed successfully");
                }
            }
            Err(e) => {
                error!("Rebalance check failed: {}", e);
            }
        }

        tokio::time::sleep(interval).await;
    }
}

async fn check_and_rebalance(
    rpc: &RpcClient,
    _keeper: &Keypair,
    vault_pubkey: &Pubkey,
    dry_run: bool,
) -> Result<bool> {
    // Fetch vault account
    let vault_account = rpc.get_account(vault_pubkey)?;
    let vault_data = &vault_account.data;

    // Skip 8-byte Anchor discriminator
    if vault_data.len() < 8 {
        return Err(anyhow::anyhow!("Invalid vault account data"));
    }

    // Deserialize vault state (simplified — in production use Anchor IDL)
    let vault = deserialize_vault_state(&vault_data[8..])?;

    // Fetch pool account
    let pool_account = rpc.get_account(&vault.pool)?;
    let pool_data = &pool_account.data;

    if pool_data.len() < 8 {
        return Err(anyhow::anyhow!("Invalid pool account data"));
    }

    let pool = deserialize_pool_state(&pool_data[8..])?;

    info!(
        "Pool tick={} | Vault range=[{}, {}] | Rebalances={}",
        pool.tick_current_index,
        vault.active_tick_lower,
        vault.active_tick_upper,
        vault.rebalance_count
    );

    if !should_rebalance(&pool, &vault) {
        return Ok(false);
    }

    let (new_lower, new_upper) = calculate_optimal_range(&pool, &vault);
    info!(
        "Rebalance needed: [{}, {}] -> [{}, {}]",
        vault.active_tick_lower, vault.active_tick_upper, new_lower, new_upper
    );

    if dry_run {
        warn!("DRY RUN — skipping transaction submission");
        return Ok(false);
    }

    // In production:
    // 1. Build rebalance instruction with new_lower, new_upper
    // 2. Simulate transaction
    // 3. Submit with priority fee via Jito or standard RPC
    // 4. Confirm transaction
    info!("Would submit rebalance tx (not implemented in keeper skeleton)");

    Ok(true)
}

/// Simplified vault state deserialization.
/// In production, use Anchor's `AccountDeserialize` or codegen from IDL.
fn deserialize_vault_state(data: &[u8]) -> Result<VaultState> {
    if data.len() < 256 {
        return Err(anyhow::anyhow!("Vault data too short"));
    }

    let authority = Pubkey::try_from(&data[0..32]).unwrap();
    let keeper = Pubkey::try_from(&data[32..64]).unwrap();
    let pool = Pubkey::try_from(&data[64..96]).unwrap();
    // skip share_mint (32), token_vault_a (32), token_vault_b (32) = 96 bytes
    // skip active_position (32) = offset 224
    let offset = 96 + 32 + 32 + 32 + 32; // = 224
    let active_tick_lower = i32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
    let active_tick_upper =
        i32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());
    let has_active_position = data[offset + 8] != 0;
    let rebalance_threshold_bps =
        u16::from_le_bytes(data[offset + 9..offset + 11].try_into().unwrap());
    let tick_range_multiplier = data[offset + 11];

    // Skip ahead to rebalance_count (much further in the struct)
    // This is simplified — real implementation would use proper offsets
    let rebalance_count = 0u32;
    let paused = false;

    Ok(VaultState {
        authority,
        keeper,
        pool,
        active_tick_lower,
        active_tick_upper,
        has_active_position,
        rebalance_threshold_bps,
        tick_range_multiplier,
        rebalance_count,
        paused,
    })
}

fn deserialize_pool_state(data: &[u8]) -> Result<PoolState> {
    if data.len() < 200 {
        return Err(anyhow::anyhow!("Pool data too short"));
    }

    // Skip authority (32), token_mint_a (32), token_mint_b (32),
    // token_vault_a (32), token_vault_b (32) = 160 bytes
    let offset = 160;
    let sqrt_price =
        u128::from_le_bytes(data[offset..offset + 16].try_into().unwrap());
    let tick_current_index =
        i32::from_le_bytes(data[offset + 16..offset + 20].try_into().unwrap());
    let tick_spacing =
        u16::from_le_bytes(data[offset + 20..offset + 22].try_into().unwrap());
    let liquidity =
        u128::from_le_bytes(data[offset + 22..offset + 38].try_into().unwrap());

    Ok(PoolState {
        sqrt_price,
        tick_current_index,
        tick_spacing,
        liquidity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_rebalance_in_range() {
        let pool = PoolState {
            sqrt_price: 0,
            tick_current_index: 500,
            tick_spacing: 10,
            liquidity: 1000,
        };
        let vault = VaultState {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 1000,
            has_active_position: true,
            rebalance_threshold_bps: 1000, // 10%
            tick_range_multiplier: 10,
            rebalance_count: 0,
            paused: false,
        };

        // tick 500 is in the middle of [0, 1000] — no rebalance needed
        assert!(!should_rebalance(&pool, &vault));
    }

    #[test]
    fn test_should_rebalance_out_of_range() {
        let pool = PoolState {
            sqrt_price: 0,
            tick_current_index: 1500,
            tick_spacing: 10,
            liquidity: 1000,
        };
        let vault = VaultState {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 1000,
            has_active_position: true,
            rebalance_threshold_bps: 1000,
            tick_range_multiplier: 10,
            rebalance_count: 0,
            paused: false,
        };

        assert!(should_rebalance(&pool, &vault));
    }

    #[test]
    fn test_should_rebalance_near_edge() {
        let pool = PoolState {
            sqrt_price: 0,
            tick_current_index: 950, // near upper edge of [0, 1000]
            tick_spacing: 10,
            liquidity: 1000,
        };
        let vault = VaultState {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 1000,
            has_active_position: true,
            rebalance_threshold_bps: 1000, // 10% threshold = 100 ticks
            tick_range_multiplier: 10,
            rebalance_count: 0,
            paused: false,
        };

        // 950 > 1000 - 100 = 900 → should rebalance
        assert!(should_rebalance(&pool, &vault));
    }

    #[test]
    fn test_calculate_optimal_range() {
        let pool = PoolState {
            sqrt_price: 0,
            tick_current_index: 500,
            tick_spacing: 10,
            liquidity: 1000,
        };
        let vault = VaultState {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 1000,
            has_active_position: true,
            rebalance_threshold_bps: 1000,
            tick_range_multiplier: 5, // 5 * 10 = 50 ticks each side
            rebalance_count: 0,
            paused: false,
        };

        let (lower, upper) = calculate_optimal_range(&pool, &vault);
        assert_eq!(lower, 450); // (500 - 50) / 10 * 10
        assert_eq!(upper, 550); // (500 + 50) / 10 * 10
    }

    #[test]
    fn test_no_rebalance_when_paused() {
        let pool = PoolState {
            sqrt_price: 0,
            tick_current_index: 2000,
            tick_spacing: 10,
            liquidity: 1000,
        };
        let vault = VaultState {
            authority: Pubkey::default(),
            keeper: Pubkey::default(),
            pool: Pubkey::default(),
            active_tick_lower: 0,
            active_tick_upper: 1000,
            has_active_position: true,
            rebalance_threshold_bps: 1000,
            tick_range_multiplier: 10,
            rebalance_count: 0,
            paused: true,
        };

        assert!(!should_rebalance(&pool, &vault));
    }
}

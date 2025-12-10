use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

/// Pool metrics collected by the indexer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolMetrics {
    pub pool_address: String,
    pub sqrt_price: u128,
    pub tick_current: i32,
    pub liquidity: u128,
    pub fee_growth_global_a: u128,
    pub fee_growth_global_b: u128,
    pub num_positions: u32,
    pub timestamp: i64,
}

/// Vault metrics collected by the indexer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultMetrics {
    pub vault_address: String,
    pub total_shares: u64,
    pub total_value_a: u64,
    pub total_value_b: u64,
    pub active_tick_lower: i32,
    pub active_tick_upper: i32,
    pub rebalance_count: u32,
    pub share_price: f64,
    pub timestamp: i64,
}

/// APY calculation over a time window.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApyData {
    pub vault_address: String,
    pub apy_7d: f64,
    pub apy_30d: f64,
    pub total_fees_earned_a: u64,
    pub total_fees_earned_b: u64,
}

/// Runs the indexer loop — polls pool and vault accounts, logs metrics.
/// In production, this would write to PostgreSQL/TimescaleDB and serve via GraphQL.
pub async fn run_indexer(rpc: Arc<RpcClient>, vault_pubkey: Pubkey) -> Result<()> {
    info!("Indexer starting for vault: {}", vault_pubkey);

    let poll_interval = std::time::Duration::from_secs(10);

    loop {
        match collect_metrics(&rpc, &vault_pubkey) {
            Ok(metrics) => {
                info!(
                    "Vault metrics: shares={} value_a={} value_b={} rebalances={}",
                    metrics.total_shares,
                    metrics.total_value_a,
                    metrics.total_value_b,
                    metrics.rebalance_count,
                );

                // In production: write to DB
                // db::insert_vault_metrics(&metrics).await?;
            }
            Err(e) => {
                tracing::error!("Failed to collect metrics: {}", e);
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

fn collect_metrics(rpc: &RpcClient, vault_pubkey: &Pubkey) -> Result<VaultMetrics> {
    let account = rpc.get_account(vault_pubkey)?;
    let data = &account.data;

    if data.len() < 264 {
        return Err(anyhow::anyhow!("Invalid vault account data"));
    }

    // Skip 8-byte discriminator + fixed fields to reach accounting section
    // This is a simplified version — production would use Anchor IDL deserialization
    let offset = 8 + 32 * 6 + 32 + 4 + 4 + 1 + 2 + 1 + 2; // approximate offset to accounting
    let total_shares = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
    let total_value_a =
        u64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap_or([0; 8]));
    let total_value_b =
        u64::from_le_bytes(data[offset + 16..offset + 24].try_into().unwrap_or([0; 8]));

    let share_price = if total_shares > 0 {
        (total_value_a as f64 + total_value_b as f64) / total_shares as f64
    } else {
        1.0
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    Ok(VaultMetrics {
        vault_address: vault_pubkey.to_string(),
        total_shares,
        total_value_a,
        total_value_b,
        active_tick_lower: 0,
        active_tick_upper: 0,
        rebalance_count: 0,
        share_price,
        timestamp: now,
    })
}

/// Calculate APY from historical share price data.
/// In production, this reads from the database.
pub fn calculate_apy(
    share_price_now: f64,
    share_price_then: f64,
    days_elapsed: f64,
) -> f64 {
    if days_elapsed <= 0.0 || share_price_then <= 0.0 {
        return 0.0;
    }

    let returns = (share_price_now - share_price_then) / share_price_then;
    let annualized = returns * (365.0 / days_elapsed);
    annualized * 100.0 // as percentage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apy_calculation() {
        // 1% return over 7 days ≈ 52.14% APY
        let apy = calculate_apy(1.01, 1.00, 7.0);
        assert!((apy - 52.14).abs() < 1.0);
    }

    #[test]
    fn test_apy_zero_days() {
        let apy = calculate_apy(1.01, 1.00, 0.0);
        assert_eq!(apy, 0.0);
    }

    #[test]
    fn test_apy_negative_return() {
        let apy = calculate_apy(0.99, 1.00, 7.0);
        assert!(apy < 0.0);
    }
}

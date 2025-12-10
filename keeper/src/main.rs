use anyhow::Result;
use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::read_keypair_file;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

mod indexer;
mod rebalancer;

#[derive(Parser, Debug)]
#[command(name = "tidepool-keeper", about = "TidePool Rebalance Keeper Bot")]
struct Args {
    /// RPC endpoint URL
    #[arg(long, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,

    /// Path to keeper keypair
    #[arg(long, default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Vault address to monitor
    #[arg(long)]
    vault: String,

    /// Poll interval in milliseconds
    #[arg(long, default_value = "5000")]
    poll_interval_ms: u64,

    /// Enable indexer mode
    #[arg(long, default_value = "false")]
    indexer: bool,

    /// Dry run — simulate but don't submit transactions
    #[arg(long, default_value = "false")]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    info!("TidePool Keeper Bot starting...");
    info!("RPC: {}", args.rpc_url);
    info!("Vault: {}", args.vault);

    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        args.rpc_url.clone(),
        CommitmentConfig::confirmed(),
    ));

    let keypair_path = shellexpand::tilde(&args.keypair).to_string();
    let keeper_keypair = read_keypair_file(&keypair_path)
        .map_err(|e| anyhow::anyhow!("Failed to read keypair: {}", e))?;

    let vault_pubkey = Pubkey::from_str(&args.vault)?;

    info!(
        "Keeper pubkey: {}",
        keeper_keypair.pubkey()
    );

    if args.indexer {
        info!("Starting indexer mode...");
        indexer::run_indexer(rpc_client.clone(), vault_pubkey).await?;
    } else {
        info!(
            "Starting rebalancer (poll_interval={}ms, dry_run={})...",
            args.poll_interval_ms, args.dry_run
        );
        rebalancer::run_rebalancer(
            rpc_client,
            keeper_keypair,
            vault_pubkey,
            args.poll_interval_ms,
            args.dry_run,
        )
        .await?;
    }

    Ok(())
}

//! Example of querying logs from the Ethereum network.

use alloy::{
    primitives::{address, b256},
    providers::{Provider, ProviderBuilder},
    rpc::types::Filter,
};
use eyre::Result;
use tempo_alloy::TempoNetwork;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a provider.
    let rpc_url = "https://rpc.moderato.tempo.xyz".parse()?;
    let provider = ProviderBuilder::new_with_network::<TempoNetwork>().connect_http(rpc_url);
    // Get all logs from the latest block that match the transfer event signature/topic.
    let transfer_event_signature =
        b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
    let filter = Filter::new()
        .event_signature(transfer_event_signature)
        .from_block(1_000_000)
        .to_block(1_000_010);
    // You could also use the event name instead of the event signature like so:
    // .event("Transfer(address,address,uint256)")

    // Get all logs from the latest block that match the filter.
    let logs = provider.get_logs(&filter).await?;

    for log in logs {
        println!("Transfer event: {log:?}");
    }

    Ok(())
}

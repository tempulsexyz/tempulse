use alloy::providers::{
    ProviderBuilder, RootProvider,
    fillers::{ChainIdFiller, GasFiller, JoinFill, NonceFiller},
};
use tempo_alloy::TempoNetwork;

/// The Tempo RPC provider type used throughout the application.
///
/// This is the filled provider returned by `ProviderBuilder::new_with_network()`.
pub type TempoProvider = alloy::providers::fillers::FillProvider<
    JoinFill<alloy::providers::Identity, JoinFill<NonceFiller, JoinFill<GasFiller, ChainIdFiller>>>,
    RootProvider<TempoNetwork>,
    TempoNetwork,
>;

/// Create a Tempo-specific HTTP provider from an RPC URL string.
pub fn create_provider(rpc_url: &str) -> eyre::Result<TempoProvider> {
    let url = rpc_url.parse()?;
    let provider = ProviderBuilder::new_with_network::<TempoNetwork>().connect_http(url);
    Ok(provider)
}

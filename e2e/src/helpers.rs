use std::{
    fmt::Debug,
    path::PathBuf,
};

use anyhow::Context;
use ethers_core::types::transaction::response::Transaction as EthersTransaction;
use log::debug;
use serde::{Deserialize, Serialize};

use zeth_lib::{
    builder::BlockBuilderStrategy,
    consts::ChainSpec,
    host::{preflight::Preflight, verify::Verifier},
    input::BlockBuildInput,
    output::BlockBuildOutput,
};
use zeth_lib::host::provider_db::ProviderDb;
use zeth_primitives::block::Header;

pub fn init() {
    let _ = env_logger::try_init();
}

/// Build a single block using the specified strategy.
pub async fn build_block<N: BlockBuilderStrategy>(
    block_no: u64,
    rpc_cache: Option<PathBuf>,
    chain_spec: &ChainSpec,
) -> anyhow::Result<BlockBuildOutput>
where
    N::TxEssence: 'static + Send + TryFrom<EthersTransaction> + Serialize + Deserialize<'static>,
    <N::TxEssence as TryFrom<EthersTransaction>>::Error: Debug,
{
    let init_spec = chain_spec.clone();
    let preflight_result = tokio::task::spawn_blocking(move || {
        N::preflight_with_external_data(&init_spec, rpc_cache, None, block_no)
    }).await?;
    let preflight_data = preflight_result.context("preflight failed")?;

    // Create the guest input from [Init]
    let input: BlockBuildInput<N::TxEssence> = preflight_data
        .clone()
        .try_into()
        .context("invalid preflight data")?;

    // Verify that the transactions run correctly
    debug!("Running from memory ...");
    let output = N::build_from(chain_spec, input).context("Error while building block")?;

    match &output {
        BlockBuildOutput::SUCCESS {
            hash, head, state, ..
        } => {
            debug!("Verifying final state using provider data ...");
            preflight_data.verify_block(head, state)?;

            debug!("Final block hash derived successfully. {}", hash);
        }
        BlockBuildOutput::FAILURE { .. } => {
            debug!("Proving bad block construction!")
        }
    }

    Ok(output)
}

pub async fn prepare_preflight_input<N: BlockBuilderStrategy>(
    chain_spec: &ChainSpec,
    provider_db: ProviderDb,
    preflight_input: BlockBuildInput<<N as BlockBuilderStrategy>::TxEssence>,
    result_block_header: Header,
) -> anyhow::Result<BlockBuildInput<N::TxEssence>>
    where
        N::TxEssence: 'static + Send + TryFrom<EthersTransaction> + Serialize + Deserialize<'static>,
        <N::TxEssence as TryFrom<EthersTransaction>>::Error: Debug,
{
    let init_spec = chain_spec.clone();
    let preflight_result = tokio::task::spawn_blocking(move || {
        N::preflight_with_local_data(&init_spec, provider_db, preflight_input)
    }).await?;
    let mut preflight_data = preflight_result.context("preflight failed")?;
    preflight_data.header = Some(result_block_header);

    let input: BlockBuildInput<N::TxEssence> = preflight_data
        .clone()
        .try_into()
        .context("invalid preflight data")?;

    Ok(input)
}

/// Build a single block using the specified strategy.
pub async fn build_block_result<N: BlockBuilderStrategy>(
    chain_spec: &ChainSpec,
    provider_db: ProviderDb,
    input: BlockBuildInput<<N as BlockBuilderStrategy>::TxEssence>,
    result_block_header: Header,
) -> anyhow::Result<BlockBuildOutput>
where
    N::TxEssence: 'static + Send + TryFrom<EthersTransaction> + Serialize + Deserialize<'static>,
    <N::TxEssence as TryFrom<EthersTransaction>>::Error: Debug,
{
    // Create the guest input from [Init]
    let input = prepare_preflight_input::<N>(chain_spec, provider_db, input, result_block_header).await?;

    // Verify that the transactions run correctly
    debug!("Running from memory ...");
    let output = N::build_from(chain_spec, input).context("Error while building block")?;

    Ok(output)
}

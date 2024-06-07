use std::fmt::Debug;
use std::path::Path;
use anyhow::Context;
use zeth_lib::builder::BlockBuilderStrategy;
use zeth_lib::consts::ChainSpec;
use zeth_lib::host::{cache_file_path, cache_file_path_exists};
use zeth_lib::host::verify::Verifier;
use zeth_lib::input::BlockBuildInput;
use zeth_lib::output::BlockBuildOutput;
use ethers_core::types::transaction::response::Transaction as EthersTransaction;
use serde::{Deserialize, Serialize};
use zeth_lib::host::preflight::Preflight;

/// Build a single block using the specified strategy.
pub async fn build_block<N: BlockBuilderStrategy>(
    rpc_url: Option<String>,
    chain_spec: &ChainSpec,
    // guest_elf: &[u8],
) -> anyhow::Result<()>
    where
        N::TxEssence: 'static + Send + TryFrom<EthersTransaction> + Serialize + Deserialize<'static>,
        <N::TxEssence as TryFrom<EthersTransaction>>::Error: Debug,
{
    let block_no = 17034871;

    // Fetch all of the initial data
    let rpc_cache = Some(cache_file_path_exists(
        Path::new("../host/testdata"),
        "ethereum",
        block_no,
        "json.gz",
    ));

    let init_spec = chain_spec.clone();
    let preflight_result = tokio::task::spawn_blocking(move || {
        N::preflight_with_external_data(&init_spec, rpc_cache, rpc_url, block_no)
    }).await?;
    let preflight_data = preflight_result.context("preflight failed")?;

    // Create the guest input from [Init]
    let input: BlockBuildInput<N::TxEssence> = preflight_data
        .clone()
        .try_into()
        .context("invalid preflight data")?;

    // Verify that the transactions run correctly
    println!("Running from memory ...");
    let output = N::build_from(chain_spec, input).context("Error while building block")?;

    match &output {
        BlockBuildOutput::SUCCESS {
            hash, head, state, ..
        } => {
            println!("Verifying final state using provider data ...");
            preflight_data.verify_block(head, state)?;

            println!("Final block hash derived successfully. {}", hash);
        }
        BlockBuildOutput::FAILURE { .. } => {
            println!("Proving bad block construction!")
        }
    }

    Ok(())
}
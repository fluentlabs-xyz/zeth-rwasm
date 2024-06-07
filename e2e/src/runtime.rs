use zeth_lib::builder::{BlockBuilder, EthereumStrategy};
use zeth_lib::builder::finalize::MemDbBlockFinalizeStrategy;
use zeth_lib::consts::{ChainSpec, ETH_MAINNET_CHAIN_SPEC, FLUENT_DEVNET_CHAIN_SPEC};
use zeth_lib::EthereumTxEssence;
use zeth_lib::input::BlockBuildInput;
use zeth_lib::mem_db::MemDb;
use crate::helpers::build_block;

#[tokio::test]
async fn runtime_test() {
    let _ = env_logger::try_init();

    build_block::<EthereumStrategy>(
        None,
        &ETH_MAINNET_CHAIN_SPEC,
        // &FLUENT_DEVNET_CHAIN_SPEC,
    ).await.expect("expected OK");

    // let mut word_reader = FluentWordReader::default();
    // let input: BlockBuildInput<EthereumTxEssence> =
    //     BlockBuildInput::deserialize(&mut Deserializer::new(&mut word_reader)).unwrap();
    // let output =
    //     BlockBuilder::<MemDb, EthereumTxEssence>::new(&FLUENT_DEVNET_CHAIN_SPEC, input)
    //         .finalize::<MemDbBlockFinalizeStrategy>().expect("failed to build block");
    // let result_hash = output.0.hash();
    // LowLevelSDK::sys_write(result_hash.as_slice());
}
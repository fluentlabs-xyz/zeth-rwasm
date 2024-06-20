#![no_std]

extern crate alloc;

use fluentbase_sdk::{LowLevelSDK, SharedAPI};
use risc0_zkvm::serde::Deserializer;
use zeth_lib::{
    builder::{BlockBuilderStrategy, EthereumStrategy},
    consts::FLUENT_DEVNET_CHAIN_SPEC,
    input::BlockBuildInput,
};
use zeth_primitives::{private::serde::Deserialize, transactions::ethereum::EthereumTxEssence};

use crate::word_reader::FluentWordReader;

mod word_reader;

#[no_mangle]
pub extern "C" fn main() {
    let mut word_reader = FluentWordReader::default();
    let input: BlockBuildInput<EthereumTxEssence> =
        BlockBuildInput::deserialize(&mut Deserializer::new(&mut word_reader))
            .expect("failed to deserialize input");
    let block_build_output = EthereumStrategy::build_from(&FLUENT_DEVNET_CHAIN_SPEC, input)
        .expect("failed to build block");
    assert!(block_build_output.success());
    let result_hash = block_build_output.hash().unwrap_or_default();
    // let output =
    //     BlockBuilder::<MemDb, EthereumTxEssence>::new(&FLUENT_DEVNET_CHAIN_SPEC, input)
    //         .finalize::<MemDbBlockFinalizeStrategy>().expect("failed to build block");
    // let result_hash = output.0.hash();
    LowLevelSDK::write(result_hash.as_ptr(), result_hash.len() as u32);
}

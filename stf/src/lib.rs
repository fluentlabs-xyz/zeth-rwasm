#![no_std]

extern crate alloc;

mod word_reader;

use fluentbase_sdk::{LowLevelAPI, LowLevelSDK};
use risc0_zkvm::serde::Deserializer;
use zeth_lib::{builder::BlockBuilder, consts::FLUENT_DEVNET_CHAIN_SPEC, input::BlockBuildInput};
use zeth_lib::builder::finalize::MemDbBlockFinalizeStrategy;
use zeth_lib::mem_db::MemDb;
use zeth_primitives::{private::serde::Deserialize, transactions::ethereum::EthereumTxEssence};

use crate::word_reader::FluentWordReader;

#[no_mangle]
// #[start]
pub extern "C" fn main() {
    let mut word_reader = FluentWordReader::default();
    let input: BlockBuildInput<EthereumTxEssence> =
        BlockBuildInput::deserialize(&mut Deserializer::new(&mut word_reader)).unwrap();
    let output =
        BlockBuilder::<MemDb, EthereumTxEssence>::new(&FLUENT_DEVNET_CHAIN_SPEC, input)
            .finalize::<MemDbBlockFinalizeStrategy>().expect("failed to build block");
    let result_hash = output.0.hash();
    LowLevelSDK::sys_write(result_hash.as_slice());
}

use std::path::Path;

use alloy_primitives::{hex, B256};
use anyhow::Context;
use ethers_core::types::{Block, Transaction};
use zeth_lib::{
    builder::{BlockBuildInput, BlockBuilderStrategy, EthereumStrategy},
    consts::ETH_MAINNET_CHAIN_SPEC,
    host::{
        cache_file_path,
        preflight::new_preflight_input,
        provider::{new_provider, BlockQuery},
        provider_db::ProviderDb,
    },
    output::BlockBuildOutput,
};
use zeth_primitives::{block::Header, transactions::ethereum::EthereumTxEssence};

use crate::helpers::{build_block, build_block_result, init, prepare_block_build_input};

#[tokio::test]
async fn proof_child_block_test() {
    init();

    let block_no = 17034871;
    let cache_path = Path::new("../host/testdata");
    let network = "ethereum";
    let cache_ext = "json.gz";

    let output = build_block::<EthereumStrategy>(
        block_no,
        Some(cache_file_path(cache_path, network, block_no, cache_ext)),
        &ETH_MAINNET_CHAIN_SPEC,
    )
    .await
    .expect("expected success");
}

#[tokio::test]
async fn build_block_using_preflight_test() {
    init();

    let expected_header_hash = B256::new(hex!(
        "075f3ef6e49e0fd1f1a8abfb715b49489d5270c4862f61b2e4f4bcebf2152b82"
    ));
    let parent_block_no = 17034870;
    let result_block_no = parent_block_no + 1;
    let network = "ethereum";
    let cache_ext = "json.gz";

    // TODO extract block test data
    let cache_file_path_string = format!(
        "../host/testdata/{}/{}.{}",
        network, result_block_no, cache_ext
    );
    let cache_file_path = Path::new(cache_file_path_string.as_str());
    let mut provider = new_provider(Some(cache_file_path.to_path_buf()), None).unwrap();

    let parent_block = provider
        .get_partial_block(&BlockQuery {
            block_no: parent_block_no,
        })
        .unwrap();
    let parent_block_header: Header = parent_block.try_into().context("invalid block").unwrap();

    let result_block = provider
        .get_full_block(&BlockQuery {
            block_no: parent_block_no + 1,
        })
        .unwrap();
    let input = new_preflight_input(parent_block_header.clone(), result_block.clone()).unwrap();

    // TODO build block using test data
    let result_block_header: Header = result_block.try_into().unwrap();
    let provider_db = ProviderDb::new(provider, parent_block_header.number);
    let block_build_output = build_block_result::<EthereumStrategy>(
        &ETH_MAINNET_CHAIN_SPEC,
        provider_db,
        input,
        result_block_header,
    )
    .await
    .unwrap();

    // TODO check expected hash against result hash

    let BlockBuildOutput::SUCCESS { hash, head, .. } = block_build_output else {
        panic!("block build output error")
    };

    assert_eq!(expected_header_hash.0, head.hash().0);
    assert_eq!(expected_header_hash.0, hash.0);
}

#[tokio::test]
async fn build_block_using_fully_initialized_block_build_input_test() {
    init();

    let expected_header_hash = B256::new(hex!(
        "075f3ef6e49e0fd1f1a8abfb715b49489d5270c4862f61b2e4f4bcebf2152b82"
    ));
    let parent_block_no = 17034870;
    let result_block_no = parent_block_no + 1;
    let network = "ethereum";
    let cache_ext = "json.gz";

    // TODO extract block test data
    let cache_file_path_string = format!(
        "../host/testdata/{}/{}.{}",
        network, result_block_no, cache_ext
    );
    let cache_file_path = Path::new(cache_file_path_string.as_str());
    let mut provider = new_provider(Some(cache_file_path.to_path_buf()), None).unwrap();

    let parent_block = provider
        .get_partial_block(&BlockQuery {
            block_no: parent_block_no,
        })
        .unwrap();
    let parent_block_header: Header = parent_block.try_into().context("invalid block").unwrap();

    let result_block = provider
        .get_full_block(&BlockQuery {
            block_no: parent_block_no + 1,
        })
        .unwrap();
    let result_block_header: Header = result_block
        .clone()
        .try_into()
        .context("invalid block")
        .unwrap();

    let provider_db = ProviderDb::new(provider, parent_block_header.number);

    let input: BlockBuildInput<EthereumTxEssence> =
        new_preflight_input(parent_block_header.clone(), result_block).unwrap();
    let input = prepare_block_build_input::<EthereumStrategy>(
        &ETH_MAINNET_CHAIN_SPEC,
        provider_db,
        input,
        result_block_header,
    )
    .await
    .unwrap();

    let block_build_output = EthereumStrategy::build_from(&ETH_MAINNET_CHAIN_SPEC, input).unwrap();
    assert!(block_build_output.success());

    assert_eq!(
        expected_header_hash.0,
        block_build_output.hash().unwrap_or_default().0
    );
}

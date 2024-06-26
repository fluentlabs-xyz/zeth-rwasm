// Copyright 2024 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::bail;
#[cfg(target_arch = "wasm32")]
use risc0_zkvm::{guest::env, serde::to_vec, sha::Digest};
use serde::{Deserialize, Serialize};
use zeth_primitives::{
    block::Header,
    mmr,
    mmr::{MerkleMountainRange, MerkleProof},
};

use crate::optimism::{batcher::BlockId, DeriveOutput};

/// Denotes a zkVM Image ID.
pub type ImageId = [u32; 8];

#[derive(Debug, Clone, Deserialize, Serialize)]
/// The input given to the composition predicate
pub struct ComposeInput {
    /// The image id used for op block building/transaction execution
    pub block_image_id: ImageId,
    /// The image id used for op block derivation
    pub derive_image_id: ImageId,
    /// The image id of the composition guest itself
    pub compose_image_id: ImageId,
    /// The “operation” which this invocation of the guest should perform
    /// (prep/lift/join/finish)
    pub operation: ComposeInputOperation,
    /// The Merkle-tree root of the Ethereum blockchain using which all derivation should
    /// be performed.
    pub eth_chain_merkle_root: mmr::Hash,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ComposeInputOperation {
    /// Takes a chain of ethereum blocks and inserts them into a Merkle-tree,
    /// resulting in a composition statement about an eth_chain_root value.
    /// This operation can also be performed incrementally when given a Merkle
    /// mountain range (and corresponding proof) as input if the ethereum chain
    /// is too large for one session.
    PREP {
        eth_blocks: Vec<Header>,
        prior_prep: Option<(ComposeOutput, MerkleMountainRange)>,
    },
    /// Lifting converts a block derivation proof into an equivalent proof, only after
    /// verifying that the last ethereum block read during derivation belongs to the
    /// continuous chain committed to under eth_chain_root. This is done using a
    /// merkle inclusion proof.
    LIFT {
        derivation: DeriveOutput,
        eth_tail_proof: MerkleProof,
    },
    /// Where the stitching logic happens. Joining can only succeed if:
    /// 1. The optimism “tail” of the left proof is equal to the optimism “head” of the
    ///    right proof (i.e. the optimism blocks in the right proof’s chain start off
    ///    where the left proof chain ends).
    /// 2. The ethereum blocks used for derivation in both proofs are on the same chain.
    ///    (security)
    /// The first condition is checked during the join operation, while the second
    /// condition is already validated during the pre-requisite “lift” operation using the
    /// eth_chain_root.
    JOIN {
        left: ComposeOutput,
        right: ComposeOutput,
    },
    /// Finalization takes as input a composition “preparation” proof about the merkle
    /// root eth_chain_root, asserting its commitment to a continuous chain of
    /// ethereum blocks, and a composition “aggregation” proof about the derivation of
    /// a series of op-blocks from the ethereum blocks under eth_chain_root.
    FINISH {
        prep: ComposeOutput,
        aggregate: ComposeOutput,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct ComposeOutput {
    pub block_image_id: ImageId,
    pub derive_image_id: ImageId,
    pub compose_image_id: ImageId,
    pub operation: ComposeOutputOperation,
    pub eth_chain_tail_block: BlockId,
    pub eth_chain_merkle_root: mmr::Hash,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub enum ComposeOutputOperation {
    PREP,
    AGGREGATE {
        op_head: BlockId,
        op_tail: BlockId,
        /// Whether the L1 block range has been validate as a correct Merkle commitment to
        /// a continuous chain of L1 blocks.
        eth_chain_continuity_validated: bool,
    },
}

impl ComposeInput {
    pub fn process(self) -> anyhow::Result<ComposeOutput> {
        let output = match self.operation {
            ComposeInputOperation::PREP {
                eth_blocks,
                prior_prep: prior,
            } => {
                // Check initial data
                let (mut eth_tail, mut mountain_range) =
                    if let Some((prior_output, prior_range)) = prior {
                        #[cfg(target_arch = "wasm32")]
                        {
                            // A valid receipt should be provided for prior aggregation
                            let compose_journal = to_vec(&prior_output)
                                .expect("Failed to encode prior aggregation journal");
                            env::verify(Digest::from(self.compose_image_id), &compose_journal)
                                .expect("Failed to validate prior aggregation");
                        }
                        // Validate context
                        assert_eq!(self.block_image_id, prior_output.block_image_id);
                        assert_eq!(self.derive_image_id, prior_output.derive_image_id);
                        assert_eq!(self.compose_image_id, prior_output.compose_image_id);
                        assert_eq!(
                            self.eth_chain_merkle_root,
                            prior_output.eth_chain_merkle_root
                        );
                        // Only append merkle range from preparation outputs
                        let ComposeOutputOperation::PREP = prior_output.operation else {
                            bail!("Unsupported! Expected ComposeOutput::PREP")
                        };

                        // Root of input mountain range should equal prior prep's root
                        assert_eq!(
                            prior_range
                                .root(None)
                                .expect("Empty mountain range used as input"),
                            self.eth_chain_merkle_root
                        );

                        (Some(prior_output.eth_chain_tail_block), prior_range)
                    } else {
                        Default::default()
                    };
                // Insert chain of blocks into mountain range
                for block in eth_blocks {
                    // Validate parent relationship
                    if let Some(BlockId {
                        hash: parent_hash, ..
                    }) = eth_tail
                    {
                        assert_eq!(block.parent_hash, parent_hash);
                    }
                    // Derive block's keccak hash
                    let block_hash = block.hash();
                    // Insert hash into mountain range
                    mountain_range.append_leaf(block_hash.0, None);
                    // Mark block as new tail
                    eth_tail.replace(BlockId {
                        number: block.number,
                        hash: block_hash,
                    });
                }

                ComposeOutput {
                    block_image_id: self.block_image_id,
                    derive_image_id: self.derive_image_id,
                    compose_image_id: self.compose_image_id,
                    operation: ComposeOutputOperation::PREP,
                    eth_chain_tail_block: eth_tail.expect("No blocks used for preparation"),
                    eth_chain_merkle_root: mountain_range.root(None).expect("Created empty range"),
                }
            }
            ComposeInputOperation::LIFT {
                derivation: derive_output,
                eth_tail_proof,
            } => {
                #[cfg(target_arch = "wasm32")]
                {
                    // Verify derivation receipt
                    let derive_journal = to_vec(&derive_output)
                        .expect("Failed to encode expected derivation journal");
                    env::verify(Digest::from(self.derive_image_id), &derive_journal)
                        .expect("Failed to lift derivation receipt");
                }
                // Verify usage of same block builder image id
                assert_eq!(self.block_image_id, derive_output.block_image_id);
                // Verify inclusion of ethereum tail in Merkle root
                assert!(
                    eth_tail_proof
                        .verify(&self.eth_chain_merkle_root, &derive_output.eth_tail.hash.0),
                    "Invalid ethereum tail inclusion proof!"
                );
                // Create output
                ComposeOutput {
                    block_image_id: self.block_image_id,
                    derive_image_id: self.derive_image_id,
                    compose_image_id: self.compose_image_id,
                    operation: ComposeOutputOperation::AGGREGATE {
                        op_head: derive_output.op_head,
                        op_tail: *derive_output
                            .derived_op_blocks
                            .last()
                            .expect("Expected at least one derived block to compose."),
                        eth_chain_continuity_validated: false,
                    },
                    eth_chain_tail_block: derive_output.eth_tail,
                    eth_chain_merkle_root: self.eth_chain_merkle_root,
                }
            }
            ComposeInputOperation::JOIN {
                left: left_compose_output,
                right: right_compose_output,
            } => {
                #[cfg(target_arch = "wasm32")]
                {
                    // Verify composition receipts
                    let left_compose_journal = to_vec(&left_compose_output)
                        .expect("Failed to encode expected left composition journal");
                    env::verify(Digest::from(self.compose_image_id), &left_compose_journal)
                        .expect("Failed to verify left composition receipt");
                    let right_compose_journal = to_vec(&right_compose_output)
                        .expect("Failed to encode expected right composition journal");
                    env::verify(Digest::from(self.compose_image_id), &right_compose_journal)
                        .expect("Failed to verify right composition receipt");
                }
                // Validate context
                // block_image_id equality
                assert_eq!(self.block_image_id, left_compose_output.block_image_id);
                assert_eq!(self.block_image_id, right_compose_output.block_image_id);
                // derive_image_id equality
                assert_eq!(self.derive_image_id, left_compose_output.derive_image_id);
                assert_eq!(self.derive_image_id, right_compose_output.derive_image_id);
                // compose_image_id equality
                assert_eq!(self.compose_image_id, left_compose_output.compose_image_id);
                assert_eq!(self.compose_image_id, right_compose_output.compose_image_id);
                // eth_chain_root equality
                assert_eq!(
                    self.eth_chain_merkle_root,
                    left_compose_output.eth_chain_merkle_root
                );
                assert_eq!(
                    self.eth_chain_merkle_root,
                    right_compose_output.eth_chain_merkle_root
                );

                // Verify op block continuity
                let ComposeOutputOperation::AGGREGATE {
                    op_head: left_op_head,
                    op_tail: left_op_tail,
                    eth_chain_continuity_validated: left_validated,
                } = left_compose_output.operation
                else {
                    bail!("Unsupported! Expected ComposeOutput::AGGREGATE")
                };
                let ComposeOutputOperation::AGGREGATE {
                    op_head: right_op_head,
                    op_tail: right_op_tail,
                    eth_chain_continuity_validated: right_validated,
                } = right_compose_output.operation
                else {
                    bail!("Unsupported! Expected ComposeOutput::AGGREGATE")
                };
                assert_eq!(&left_op_tail, &right_op_head);

                ComposeOutput {
                    block_image_id: self.block_image_id,
                    derive_image_id: self.derive_image_id,
                    compose_image_id: self.compose_image_id,
                    operation: ComposeOutputOperation::AGGREGATE {
                        op_head: left_op_head,
                        op_tail: right_op_tail,
                        eth_chain_continuity_validated: left_validated || right_validated,
                    },
                    eth_chain_tail_block: core::cmp::max(
                        left_compose_output.eth_chain_tail_block,
                        right_compose_output.eth_chain_tail_block,
                    ),
                    eth_chain_merkle_root: self.eth_chain_merkle_root,
                }
            }
            ComposeInputOperation::FINISH { prep, aggregate } => {
                // Verify prep receipt
                #[cfg(target_arch = "wasm32")]
                {
                    // A valid receipt should be provided for merkle tree prep
                    let prep_journal = to_vec(&prep).expect("Failed to encode prep journal");
                    env::verify(Digest::from(self.compose_image_id), &prep_journal)
                        .expect("Failed to validate prep receipt");
                }
                // Verify aggregate receipt
                #[cfg(target_arch = "wasm32")]
                {
                    // A valid receipt should be provided for aggregation
                    let aggregation_journal =
                        to_vec(&aggregate).expect("Failed to encode aggregation journal");
                    env::verify(Digest::from(self.compose_image_id), &aggregation_journal)
                        .expect("Failed to validate aggregate receipt");
                }
                // Validate context
                // block_image_id equality
                assert_eq!(self.block_image_id, prep.block_image_id);
                assert_eq!(self.block_image_id, aggregate.block_image_id);
                // derive_image_id equality
                assert_eq!(self.derive_image_id, prep.derive_image_id);
                assert_eq!(self.derive_image_id, aggregate.derive_image_id);
                // compose_image_id equality
                assert_eq!(self.compose_image_id, prep.compose_image_id);
                assert_eq!(self.compose_image_id, aggregate.compose_image_id);
                // eth_chain_root equality
                assert_eq!(self.eth_chain_merkle_root, prep.eth_chain_merkle_root);
                assert_eq!(self.eth_chain_merkle_root, aggregate.eth_chain_merkle_root);
                // Verify composition
                let ComposeOutputOperation::PREP = prep.operation else {
                    bail!("Unsupported! Expected ComposeOutput::PREP")
                };
                let ComposeOutputOperation::AGGREGATE {
                    op_head, op_tail, ..
                } = aggregate.operation
                else {
                    bail!("Unsupported! Expected ComposeOutput::AGGREGATE")
                };
                // Output new aggregate with validated chain root
                ComposeOutput {
                    block_image_id: self.block_image_id,
                    derive_image_id: self.derive_image_id,
                    compose_image_id: self.compose_image_id,
                    operation: ComposeOutputOperation::AGGREGATE {
                        op_head,
                        op_tail,
                        eth_chain_continuity_validated: true,
                    },
                    eth_chain_tail_block: prep.eth_chain_tail_block,
                    eth_chain_merkle_root: self.eth_chain_merkle_root,
                }
            }
        };
        Ok(output)
    }
}

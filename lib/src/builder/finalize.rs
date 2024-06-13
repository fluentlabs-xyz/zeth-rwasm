// Copyright 2023 RISC Zero, Inc.
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

use core::mem;

use anyhow::Result;
use hashbrown::HashSet;
use revm::{Database, DatabaseCommit};
use revm::interpreter::instructions::arithmetic::add;
use zeth_primitives::{
    address,
    block::Header,
    hex::FromHex,
    keccak::keccak,
    transactions::TxEssence,
    trie::{MptNode, StateAccount},
    Address, U256,
};

use crate::{
    builder::BlockBuilder,
    guest_mem_forget,
    mem_db::{AccountState, MemDb},
};

pub trait BlockFinalizeStrategy<D>
where
    D: Database + DatabaseCommit,
    <D as Database>::Error: core::fmt::Debug,
{
    fn finalize<E>(block_builder: BlockBuilder<D, E>) -> Result<(Header, MptNode)>
    where
        E: TxEssence;
}

pub struct MemDbBlockFinalizeStrategy {}

impl BlockFinalizeStrategy<MemDb> for MemDbBlockFinalizeStrategy {
    fn finalize<E: TxEssence>(
        mut block_builder: BlockBuilder<MemDb, E>,
    ) -> Result<(Header, MptNode)> {
        let db = block_builder.db.take().expect("DB not initialized");

        // apply state updates
        let mut state_trie = mem::take(&mut block_builder.input.parent_state_trie);

        // TODO stas: this account has Touched state while it must have None
        // original: address 0x9db7378614d8d9d7149c4ee4763f88c38f9b1517 account.state None
        // fluent: address 0x9db7378614d8d9d7149c4ee4763f88c38f9b1517 account.state Touched
        // #[cfg(feature = "revm-rwasm")]
        // let addresses_to_skip: HashSet<Address> =
        //     HashSet::from([
        //         address!("9db7378614d8d9d7149c4ee4763f88c38f9b1517"),
        //     ]);
        for (address, account) in &db.accounts {
            // if the account has not been touched, it can be ignored
            if account.state == AccountState::None {
                continue;
            }

            // compute the index of the current account in the state trie
            let state_trie_index = keccak(address);

            // remove deleted accounts from the state trie
            if account.state == AccountState::Deleted {
                state_trie.delete(&state_trie_index)?;
                continue;
            }

            // otherwise, compute the updated storage root for that account
            let mut state_storage = account.storage.clone();
            let mut storage_root = {
                // TODO patch example on how to fix storage_root for revm-rwasm impl
                // if address == &address!("659d9c4c146a652c6a8d3bd41f95ebc431e77533") {
                //     state_storage.insert(
                //         U256::from_str_radix("6", 10).unwrap(),
                //         U256::from_str_radix("2720784422936318780515815742981345827588049056581677080702786694440242077344", 10).unwrap(),
                //     );
                //     println!("address {} state_storage {:?}", address, &state_storage);
                // }
                // getting a mutable reference is more efficient than calling remove
                // every account must have an entry, even newly created accounts
                let (storage_trie, _) =
                    block_builder.input.parent_storage.get_mut(address).unwrap();
                // for cleared accounts always start from the empty trie
                if account.state == AccountState::StorageCleared {
                    storage_trie.clear();
                }

                // apply all new storage entries for the current account (address)
                for (key, value) in state_storage {
                    let storage_trie_index = keccak(key.to_be_bytes::<32>());
                    if value == U256::ZERO {
                        storage_trie.delete(&storage_trie_index)?;
                    } else {
                        storage_trie.insert_rlp(&storage_trie_index, value)?;
                    }
                }

                storage_trie.hash()
            };

            // TODO stas: key 6 has different value -> account has different storage root
            // ORIGINAL:
            //   address 0x659d9c4c146a652c6a8d3bd41f95ebc431e77533 account.state Touched
            //   state_storage {
            //      5: 45330885685293760170405486442225356769478331399115303630520427966370057301216,
            //      6: 2720784422936318780515815742981345827588049056581677080702786694440242077344,
            //      16921146479423737600816856300293586577363531533121648673699333122620798344256: 300,
            //      28277953007402034988080192928118807762752791787365608978775991048184084215955: 2457278976110187388957680054947937361550775857450167263402,
            //      52176543407304784673208917617381825595248219230539033735639796878474849563172: 2457278976110187388957680054947937361550775857450167263402,
            //      4212125087051585207776002848691151178117811194229561608037624018064578042108: 2457278976110187388957680054947937361550775857450167263402,
            //      43797733444888539622901775908572427551154076973391268783716871290209965545224: 1291272085159668613190,
            //      41422840562036777978735848681726229999584221306266066709303649322723019791669: 2
            //   }
            // FLUENT:
            //   address 0x659d9c4c146a652c6a8d3bd41f95ebc431e77533 account.state Touched
            //   state_storage {
            //      5: 45330885685293760170405486442225356769478331399115303630520427966370057301216,
            //      6: 2720778859528744067471080450681362515977306384946750016316355053956565524128,
            //      16921146479423737600816856300293586577363531533121648673699333122620798344256: 300,
            //      28277953007402034988080192928118807762752791787365608978775991048184084215955: 2457278976110187388957680054947937361550775857450167263402,
            //      52176543407304784673208917617381825595248219230539033735639796878474849563172: 2457278976110187388957680054947937361550775857450167263402,
            //      4212125087051585207776002848691151178117811194229561608037624018064578042108: 2457278976110187388957680054947937361550775857450167263402,
            //      43797733444888539622901775908572427551154076973391268783716871290209965545224: 1291272085159668613190,
            //      41422840562036777978735848681726229999584221306266066709303649322723019791669: 2
            //   }
            // #[cfg(feature = "revm-rwasm")]
            // if address == &address!("659d9c4c146a652c6a8d3bd41f95ebc431e77533") {
            //     storage_root = B256::from_hex(
            //         "0x16678a6e4155e6d31417b51cd9fb5cbde93d0656a37ddf2430aeace8bc8885b3",
            //     )
            //     .unwrap();
            // };

            // ORIGINAL
            // !!! address 0x659D9c4c146a652C6a8D3bD41f95ebC431E77533 new_account.storage {
            //  6: StorageSlot { previous_or_original_value: 2720783522466170224334132617124854338148813387039177686162274519834943250080, present_value: 2720784422936318780515815742981345827588049056581677080702786694440242077344 },
            //  16921146479423737600816856300293586577363531533121648673699333122620798344256: StorageSlot { previous_or_original_value: 230, present_value: 300 },
            //  41422840562036777978735848681726229999584221306266066709303649322723019791669: StorageSlot { previous_or_original_value: 2, present_value: 2 },
            //  28277953007402034988080192928118807762752791787365608978775991048184084215955: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  43797733444888539622901775908572427551154076973391268783716871290209965545224: StorageSlot { previous_or_original_value: 0, present_value: 1291272085159668613190 },
            //  5: StorageSlot { previous_or_original_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216, present_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216 },
            //  52176543407304784673208917617381825595248219230539033735639796878474849563172: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  4212125087051585207776002848691151178117811194229561608037624018064578042108: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 }}
            //
            // !!! address 0x659D9c4c146a652C6a8D3bD41f95ebC431E77533 new_account.storage {
            //  41422840562036777978735848681726229999584221306266066709303649322723019791669: StorageSlot { previous_or_original_value: 2, present_value: 2 },
            //  43797733444888539622901775908572427551154076973391268783716871290209965545224: StorageSlot { previous_or_original_value: 0, present_value: 1291272085159668613190 },
            //  4212125087051585207776002848691151178117811194229561608037624018064578042108: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  28277953007402034988080192928118807762752791787365608978775991048184084215955: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  5: StorageSlot { previous_or_original_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216, present_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216 },
            //  52176543407304784673208917617381825595248219230539033735639796878474849563172: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  16921146479423737600816856300293586577363531533121648673699333122620798344256: StorageSlot { previous_or_original_value: 230, present_value: 300 },
            //  6: StorageSlot { previous_or_original_value: 2720783522466170224334132617124854338148813387039177686162274519834943250080, present_value: 2720784422936318780515815742981345827588049056581677080702786694440242077344 }}
            //
            // FLUENT:
            // !!! address 0x659D9c4c146a652C6a8D3bD41f95ebC431E77533 new_account.storage {
            //  28277953007402034988080192928118807762752791787365608978775991048184084215955: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  52176543407304784673208917617381825595248219230539033735639796878474849563172: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  43797733444888539622901775908572427551154076973391268783716871290209965545224: EvmStorageSlot { original_value: 0, present_value: 1291272085159668613190 },
            //  6: EvmStorageSlot { original_value: 2720783522466170224334132617124854338148813387039177686162274519834943250080, present_value: 2720778859528744067471080450681362515977306384946750016316355053956565524128 },
            //  16921146479423737600816856300293586577363531533121648673699333122620798344256: EvmStorageSlot { original_value: 230, present_value: 300 },
            //  41422840562036777978735848681726229999584221306266066709303649322723019791669: EvmStorageSlot { original_value: 2, present_value: 2 },
            //  4212125087051585207776002848691151178117811194229561608037624018064578042108: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  5: EvmStorageSlot { original_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216, present_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216 }}
            //
            // !!! address 0x659D9c4c146a652C6a8D3bD41f95ebC431E77533 new_account.storage {
            //  28277953007402034988080192928118807762752791787365608978775991048184084215955: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  16921146479423737600816856300293586577363531533121648673699333122620798344256: EvmStorageSlot { original_value: 230, present_value: 300 },
            //  6: EvmStorageSlot { original_value: 2720783522466170224334132617124854338148813387039177686162274519834943250080, present_value: 2720778859528744067471080450681362515977306384946750016316355053956565524128 },
            //  41422840562036777978735848681726229999584221306266066709303649322723019791669: EvmStorageSlot { original_value: 2, present_value: 2 },
            //  43797733444888539622901775908572427551154076973391268783716871290209965545224: EvmStorageSlot { original_value: 0, present_value: 1291272085159668613190 },
            //  4212125087051585207776002848691151178117811194229561608037624018064578042108: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  5: EvmStorageSlot { original_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216, present_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216 },
            //  52176543407304784673208917617381825595248219230539033735639796878474849563172: EvmStorageSlot { original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 }}
            //
            // !!! address 0x659D9c4c146a652C6a8D3bD41f95ebC431E77533 new_account.storage {
            //  16921146479423737600816856300293586577363531533121648673699333122620798344256: StorageSlot { previous_or_original_value: 230, present_value: 300 },
            //  28277953007402034988080192928118807762752791787365608978775991048184084215955: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  52176543407304784673208917617381825595248219230539033735639796878474849563172: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  5: StorageSlot { previous_or_original_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216, present_value: 45330885685293760170405486442225356769478331399115303630520427966370057301216 },
            //  41422840562036777978735848681726229999584221306266066709303649322723019791669: StorageSlot { previous_or_original_value: 2, present_value: 2 },
            //  6: StorageSlot { previous_or_original_value: 2720783522466170224334132617124854338148813387039177686162274519834943250080, present_value: 2720784422936318780515815742981345827588049056581677080702786694440242077344 },
            //  4212125087051585207776002848691151178117811194229561608037624018064578042108: StorageSlot { previous_or_original_value: 0, present_value: 2457278976110187388957680054947937361550775857450167263402 },
            //  43797733444888539622901775908572427551154076973391268783716871290209965545224: StorageSlot { previous_or_original_value: 0, present_value: 1291272085159668613190 }}

            let state_account = StateAccount {
                nonce: account.info.nonce,
                balance: account.info.balance,
                storage_root,
                code_hash: account.info.code_hash,
            };
            // println!("address {:?} state_account {:?}", address, &state_account);
            state_trie.insert_rlp(&state_trie_index, state_account)?;
        }

        // update result header with the new state root
        let mut header = block_builder.header.take().expect("Header not initialized");
        header.state_root = state_trie.hash();

        // Leak memory, save cycles
        guest_mem_forget(block_builder);

        Ok((header, state_trie))
    }
}

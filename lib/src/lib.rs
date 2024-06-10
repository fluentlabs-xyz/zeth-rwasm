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

extern crate core;

#[cfg(not(target_arch = "wasm32"))]
pub mod host;

pub mod builder;
pub mod consts;
pub mod input;
pub mod mem_db;
// pub mod optimism;
pub mod output;

mod utils;

pub use zeth_primitives::transactions::{ethereum::EthereumTxEssence, optimism::OptimismTxEssence};

cfg_if::cfg_if! {
    if #[cfg(all(not(feature = "hashbrown"), feature = "std"))] {
        pub use std::collections::{hash_map, hash_set, HashMap, HashSet};
        use hashbrown as _;
    } else {
        pub use hashbrown::{hash_map, hash_set, HashMap, HashSet};
    }
}

/// call forget only if running inside the guest
pub fn guest_mem_forget<T>(_t: T) {
    #[cfg(target_arch = "wasm32")]
    core::mem::forget(_t)
}

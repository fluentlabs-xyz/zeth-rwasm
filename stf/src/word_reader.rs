use core::slice::from_raw_parts_mut;

use fluentbase_sdk::{LowLevelSDK, SharedAPI};
use risc0_zkvm::{serde::WordRead, sha::WORD_SIZE};

#[derive(Default)]
pub struct FluentWordReader(u32);

impl WordRead for FluentWordReader {
    fn read_words(&mut self, words: &mut [u32]) -> risc0_zkvm::serde::Result<()> {
        let mut words_u8 =
            unsafe { from_raw_parts_mut(words.as_mut_ptr() as *mut u8, words.len() * WORD_SIZE) };
        LowLevelSDK::read(&mut words_u8, self.0);
        self.0 += words.len() as u32 * 4;
        Ok(())
    }

    fn read_padded_bytes(&mut self, bytes: &mut [u8]) -> risc0_zkvm::serde::Result<()> {
        LowLevelSDK::read(bytes, self.0);
        self.0 += bytes.len() as u32;
        let unaligned = bytes.len() % WORD_SIZE;
        if unaligned != 0 {
            let pad_bytes = WORD_SIZE - unaligned;
            let mut padding = [0u8; WORD_SIZE];
            LowLevelSDK::write(&mut padding[..pad_bytes]);
            self.0 += pad_bytes as u32;
        }
        Ok(())
    }
}

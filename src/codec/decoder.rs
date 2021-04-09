#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder,
};

use super::encoder::{
    BlockInfo,
    EncodedBlock,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaptorQDecoderError {
    /// TODO: make errors more useful. 
    BadBlockId,
    RaptorQDecodeFailed,
}

/// A representation of a BlockDecoder
pub struct BlockDecoder {
    /// Block metadata
    block_info: BlockInfo,
}

impl BlockDecoder {
    pub fn new(block_info: BlockInfo) -> Result<BlockDecoder, RaptorQDecoderError> {
        return Ok(BlockDecoder{block_info: block_info});
    }

    fn extract_packets(mut blocks: Vec<EncodedBlock>, packets:&mut Vec<EncodingPacket>, block_id: u32) -> Option<RaptorQDecoderError> {
        while match blocks.pop() {
            None => false,
            Some(block) => {
                if block_id != block.block_id {
                    return Some(RaptorQDecoderError::BadBlockId);
                }
                packets.push(block.data);
                true
            },
        } {}

        return None;
    }

    /// static method for encoding data
    pub(crate) fn decode_data(block_info: BlockInfo, mut blocks: Vec<EncodedBlock>) -> Result<Vec<u8>, RaptorQDecoderError> {
        let mut decoder = SourceBlockDecoder::new2(0, &block_info.config, block_info.padded_size as u64);
        let mut packets: Vec<EncodingPacket> = Vec::new();

        match BlockDecoder::extract_packets(blocks, &mut packets, block_info.block_id) {
            Some(error) => return Err(error),
            None => (),
        }

        match decoder.decode(packets) {
            None => return Err(RaptorQDecoderError::RaptorQDecodeFailed),
            Some(data) => return Ok(data)
        }
    }
}


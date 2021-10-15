#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder,
};
use std::collections::HashSet;

use super::encoder::{
    BlockInfo,
    EncodedBlock,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaptorQDecoderError {
    /// TODO: make errors more useful. 
    BadBlockId,
    RaptorQDecodeFailed,
    BadBlockInfo
}

pub struct RaptorQDecoder {
    block_decoders: Vec<BlockDecoder>,
    blocks: Vec<Vec<EncodedBlock>>,

}

impl RaptorQDecoder {
    pub fn new(block_info_vec: Vec<BlockInfo>) -> Result<RaptorQDecoder, RaptorQDecoderError> {
        // validate the block info vector, it should be a permutation of (0..num_blocks-1)
        // pepega strat: sort and assert equality to 1..block_info_vec.len()
        let block_ids: Vec<usize> = block_info_vec.iter().map(|block_info| block_info.block_id as usize).collect();
        if block_ids != (0..block_info_vec.len()).collect::<Vec<usize>>() {
            return Err(RaptorQDecoderError::BadBlockInfo);
        }

        let block_decoder_results: Result<Vec<BlockDecoder>, RaptorQDecoderError> = block_info_vec.into_iter().map(|block_info| BlockDecoder::new(block_info)).collect();
        match block_decoder_results {
            Ok(block_decoders) => return Ok(RaptorQDecoder{block_decoders: block_decoders, blocks: Vec::with_capacity(block_decoders.len())}),
            Err(error) => return Err(error),
        }
    }

    fn add_block(&mut self, block: EncodedBlock) -> usize {
        if (block.block_id as usize) < self.blocks.len() {
            self.blocks[block.block_id as usize].push(block);
            return 1;
        }

        return 0;
    }

    /// consume some blocks into the decoder, report back how many blocks have been consumed
    pub fn add_blocks(&mut self, blocks: Vec<EncodedBlock>) -> usize {
        return blocks.into_iter().map(|block| self.add_block(block)).sum();
    }
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
    pub(crate) fn decode_data(block_info: &BlockInfo, mut blocks: Vec<EncodedBlock>) -> Result<Vec<u8>, RaptorQDecoderError> {
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

    pub fn decode_blocks(&self, mut blocks: Vec<EncodedBlock>) -> Result<Vec<u8>, RaptorQDecoderError> {
        return BlockDecoder::decode_data(&self.block_info, blocks);
    }
}

#[cfg(test)]
use super::encoder::*;
mod tests {
    use super::*;
    use rand::Rng;

    fn gen_data(len: usize) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::with_capacity(len);
        for _ in 0..len {
            data.push(rand::thread_rng().gen());
        }
        return data;
    }
    
    fn arr_eq(data1: &[u8], data2: &[u8]) -> bool {
        return data1.iter().zip(data2.iter()).all(|(a,b)| a == b);
    }

    #[test]
    fn test_block_decode_single_client() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);
        
        let encoder = match BlockEncoder::new(0, packet_size, data.clone()) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };

        let blocks = encoder.generate_encoded_blocks();
        
        let decoder = match BlockDecoder::new(encoder.get_block_info()) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };

        match decoder.decode_blocks(blocks) {
            Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
            Err(error) => panic!("Failed to decode data, err {}", error as u32),
        }
    }
}
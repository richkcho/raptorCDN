#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, SourceBlockDecoder,
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
    BadBlockInfo
}

pub struct RaptorQDecoder {
    block_decoder_data: Vec<(BlockDecoder, Vec<EncodedBlock>)>,
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
            Ok(block_decoders) => return Ok(RaptorQDecoder{block_decoder_data: block_decoders.into_iter().map(|x| (x, Vec::new())).collect()}),
            Err(error) => return Err(error),
        }
    }

    fn consume_block(&mut self, block: EncodedBlock) -> usize {
        if (block.block_id as usize) < self.block_decoder_data.len() {
            self.block_decoder_data[block.block_id as usize].1.push(block);
            return 1;
        }

        return 0;
    }

    /// consume some blocks into the decoder, report back how many blocks have been consumed
    pub fn consume_blocks(&mut self, blocks: Vec<EncodedBlock>) -> usize {
        return blocks.into_iter().map(|block| self.consume_block(block)).sum();
    }

    /// Attempt to decode the blocks. 
    pub fn decode_blocks(&mut self) -> Result<Vec<u8>, RaptorQDecoderError> {
        return match self.block_decoder_data.iter().map(|(decoder, blocks)| decoder.decode_blocks(blocks.to_vec())).collect::<Result<Vec<Vec<u8>>, RaptorQDecoderError>>() {
            Ok(block_data_vec) => Ok(block_data_vec.into_iter().flatten().collect()),
            Err(err) => Err(err),
        }
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

    fn extract_packet(block: EncodedBlock, block_id: u32) -> Result<EncodingPacket, RaptorQDecoderError> {
        if block.block_id != block_id {
            return Err(RaptorQDecoderError::BadBlockId);
        }

        return Ok(block.data);
    }

    // the raptorq wants an owned Vec<EncodingPacket>, so we create this for it. 
    fn extract_packets(blocks: Vec<EncodedBlock>, block_id: u32) -> Result<Vec<EncodingPacket>, RaptorQDecoderError> {
        return blocks.into_iter().map(|block| BlockDecoder::extract_packet(block, block_id)).collect();
    }

    /// static method for decoding data
    pub(crate) fn decode_data(block_info: &BlockInfo, blocks: Vec<EncodedBlock>) -> Result<Vec<u8>, RaptorQDecoderError> {
        let mut decoder = SourceBlockDecoder::new2(0, &block_info.config, block_info.padded_size as u64);

        let packets = match BlockDecoder::extract_packets(blocks, block_info.block_id) {
            Ok(foo) => foo,
            Err(err) => return Err(err),
        };

        let mut decoded_data = match decoder.decode(packets) {
            None => return Err(RaptorQDecoderError::RaptorQDecodeFailed),
            Some(data) => data
        };

        assert_eq!(decoded_data.len(), block_info.padded_size);
        decoded_data.truncate(block_info.payload_size);

        return Ok(decoded_data);
    }

    /// consume and decode blocks according to the BlockInfo associated with this BlockDecoder
    pub fn decode_blocks(&self, blocks: Vec<EncodedBlock>) -> Result<Vec<u8>, RaptorQDecoderError> {
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
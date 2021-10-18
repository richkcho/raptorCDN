use raptorq::{EncodingPacket, SourceBlockDecoder};
use rayon::prelude::*;
use anyhow;
use anyhow::Result;
use super::encoder::{BlockInfo, EncodedBlock};

pub struct RaptorQDecoder {
    block_decoder_data: Vec<(BlockDecoder, Vec<EncodedBlock>)>,
}

impl RaptorQDecoder {
    pub fn new(block_info_vec: Vec<BlockInfo>) -> Result<RaptorQDecoder> {
        // validate the block info vector, it should be a permutation of (0..num_blocks-1)
        // pepega strat: sort and assert equality to 1..block_info_vec.len()
        let block_ids: Vec<usize> = block_info_vec
            .iter()
            .map(|block_info| block_info.block_id as usize)
            .collect();
        if block_ids != (0..block_info_vec.len()).collect::<Vec<usize>>() {
            return Err(anyhow::anyhow!("RaptorQDecoder error: bad / incomplete block id set"));
        }

        Ok(RaptorQDecoder {
            block_decoder_data: block_info_vec
                .into_iter()
                .map(BlockDecoder::new)
                .collect::<Result<Vec<BlockDecoder>>>()?
                .into_iter()
                .map(|x| (x, Vec::new()))
                .collect(),
        })
    }

    fn consume_block(&mut self, block: EncodedBlock) -> usize {
        if (block.block_id as usize) < self.block_decoder_data.len() {
            self.block_decoder_data[block.block_id as usize]
                .1
                .push(block);
            return 1;
        }

        0
    }

    /// consume some blocks into the decoder, report back how many blocks have been consumed
    pub fn consume_blocks(&mut self, blocks: Vec<EncodedBlock>) -> usize {
        blocks
            .into_iter()
            .map(|block| self.consume_block(block))
            .sum()
    }

    /// Attempt to decode the blocks.
    pub fn decode_blocks(&mut self) -> Result<Vec<u8>> {
        Ok(self
            .block_decoder_data
            .par_iter()
            .map(|(decoder, blocks)| decoder.decode_blocks(blocks.to_vec()))
            .collect::<Result<Vec<Vec<u8>>>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}

/// A representation of a BlockDecoder
pub struct BlockDecoder {
    /// Block metadata
    block_info: BlockInfo,
}

impl BlockDecoder {
    pub fn new(block_info: BlockInfo) -> Result<BlockDecoder> {
        Ok(BlockDecoder { block_info })
    }

    fn extract_packet(
        block: EncodedBlock,
        block_id: u32,
    ) -> Result<EncodingPacket> {
        if block.block_id != block_id {
            return Err(anyhow::anyhow!("BlockDecoder error: block id mismatch!"));
        }

        Ok(block.data)
    }

    // the raptorq wants an owned Vec<EncodingPacket>, so we create this for it.
    fn extract_packets(
        blocks: Vec<EncodedBlock>,
        block_id: u32,
    ) -> Result<Vec<EncodingPacket>> {
        blocks
            .into_iter()
            .map(|block| BlockDecoder::extract_packet(block, block_id))
            .collect()
    }

    /// static method for decoding data
    pub fn decode_data(
        block_info: &BlockInfo,
        blocks: Vec<EncodedBlock>,
    ) -> Result<Vec<u8>> {
        let mut decoder =
            SourceBlockDecoder::new2(0, &block_info.config, block_info.padded_size as u64);

        let mut decoded_data = decoder
            .decode(BlockDecoder::extract_packets(blocks, block_info.block_id)?)
            .ok_or(anyhow::anyhow!("BlockDecoder error: block decode failed!"))?;

        assert_eq!(decoded_data.len(), block_info.padded_size);
        decoded_data.truncate(block_info.payload_size);

        Ok(decoded_data)
    }

    /// consume and decode blocks according to the BlockInfo associated with this BlockDecoder
    pub fn decode_blocks(&self, blocks: Vec<EncodedBlock>) -> Result<Vec<u8>> {
        BlockDecoder::decode_data(&self.block_info, blocks)
    }
}

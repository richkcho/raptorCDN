#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{EncodingPacket, ObjectTransmissionInformation, SourceBlockEncoder, SourceBlockEncodingPlan};
use std::cmp;
use super::consts::*;
use rand::{thread_rng, Rng};
use rayon::prelude::*;

pub struct RaptorQEncoder {
    block_encoders: Vec<BlockEncoder>,
}

/// RaptorQ data encoder. 
impl RaptorQEncoder {
    pub fn new(packet_size: u16, data: &[u8]) -> Result<RaptorQEncoder, RaptorQEncoderError> {
        let block_size = MAX_SYMBOLS_IN_BLOCK * packet_size as usize;

        let data_chunks: Vec<Vec<u8>> = data.chunks(block_size).map(|x| x.to_vec()).collect();

        let encoder_results: Result<Vec<BlockEncoder>, RaptorQEncoderError> = data_chunks.par_iter().enumerate().map(|(i, data_chunk)| BlockEncoder::new(i as u32, packet_size, data_chunk.to_vec())).collect();

        match encoder_results {
            Ok(block_encoders) =>
                return Ok(RaptorQEncoder {
                block_encoders: block_encoders,
            }),
            Err(error) => return Err(error),
        };
    }

    pub fn generate_encoded_blocks(&self) -> Vec<EncodedBlock> {
        return self.block_encoders.par_iter().map(|encoder| encoder.generate_encoded_blocks()).flatten().collect();
    }

    pub fn get_block_info_vec(&self) -> Vec<BlockInfo> {
        return self.block_encoders.par_iter().map(|x| x.get_block_info()).collect();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaptorQEncoderError {
    /// Packet size provided is not valid. 
    /// TODO: make errors more useful. 
    InvalidPacketSize,
    DataSizeTooLarge,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct EncodedBlock {
    /// Index of this block in overall payload
    pub block_id: u32,
    /// raptorq packet
    pub data: EncodingPacket,
}

/// Information about the payload encoded by a BlockEncoder. Needs to be transmitted from the encoder to the decoder.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct BlockInfo {
    /// Size of payload in data.
    pub payload_size: usize,
    // Actual size of data, including padding.
    pub padded_size: usize,
    /// RaptorQ configuration object
    pub config: ObjectTransmissionInformation,
    // Index of this block in overall payload. 
    pub block_id: u32,
}

/// A representation of a BlockEncoder
pub struct BlockEncoder {
    /// RaptorQ configuration object
    config: ObjectTransmissionInformation,
    /// Data to be encoded with the RaptorQ scheme (padded to a multiple of packet_size)
    data: Vec<u8>,
    /// Original size of data before padding.
    payload_size: usize,
    /// Index of this block in overall payload.
    block_id: u32,
    /// Encoded packet size. Also the symbol size used for BlockEncoder.
    packet_size: u16,
    /// Encoding plan - helps for small RAPTORQ_MAX_SYMBOLS_IN_BLOCK values.
    /// TODO: validate with real data. 
    encoding_plan: SourceBlockEncodingPlan,
}

impl BlockEncoder {
    /// Creates a BlockEncoder with a given data payload and packet size
    /// We use packet size == symbol size. 
    pub fn new(block_id: u32, packet_size: u16, mut data: Vec<u8>) -> Result<BlockEncoder, RaptorQEncoderError> {
        if packet_size % ALIGNMENT as u16 != 0 || packet_size < MIN_PACKET_SIZE {
            return Err(RaptorQEncoderError::InvalidPacketSize);
        }

        let payload_size = data.len();

        // The rust RaptorQ library asserts data length to be a multiple of packet size, pad with zeros.
        if data.len() % packet_size as usize > 0 {
            data.resize(
                data.len() + (packet_size as usize - (data.len() % packet_size as usize)),
                0,
            );
        }

        let num_symbols = data.len() / packet_size as usize;

        if num_symbols > MAX_SYMBOLS_IN_BLOCK {
            return Err(RaptorQEncoderError::DataSizeTooLarge);
        }

        let plan = SourceBlockEncodingPlan::generate(num_symbols as u16);

        /*
         * ObjectTransmissionInformation is described roughly by the RFC spec:
         * RFC 4.4.1.2:
         * The construction of source blocks and sub-blocks is determined based
         * on five input parameters -- F, Al, T, Z, and N -- and a function
         * Partition[].  The five input parameters are defined as follows:
         * - F: the transfer length of the object, in octets
         * - T: the symbol size, in octets, which MUST be a multiple of Al
         * - Z: the number of source blocks
         * - N: the number of sub-blocks in each source block
         * - Al: a symbol alignment parameter, in octets
         *
         * Notes:
         * Consider tweaking the sub-block argument.
         */
        return Ok(BlockEncoder {
            config: ObjectTransmissionInformation::new(
                data.len() as u64,
                packet_size,
                1,
                1,
                ALIGNMENT,
            ),
            data: data,
            payload_size: payload_size,
            packet_size: packet_size,
            block_id: block_id,
            encoding_plan: plan,
        });
    }

    fn add_packets(blocks:&mut Vec<EncodedBlock>, mut packets: Vec<EncodingPacket>, block_id: u32) {
        while match packets.pop() {
            None => false,
            Some(packet) => {
                blocks.push(EncodedBlock{block_id: block_id, data: packet});
                true
            },
        } {}
    }

    /// static method for encoding data
    pub fn encode_data(config: &ObjectTransmissionInformation, plan: &SourceBlockEncodingPlan, data: &[u8], packet_size: u16, block_id: u32) -> Vec<EncodedBlock> {
        let encoder = SourceBlockEncoder::with_encoding_plan2(0, config, data, plan);
        let packets_to_send = data.len() / packet_size as usize;
        let mut blocks :Vec<EncodedBlock> = Vec::new();

        let start_index = thread_rng().gen_range(0..RAPTORQ_ENCODING_SYMBOL_ID_MAX);
        
        let packets_created = cmp::min(RAPTORQ_ENCODING_SYMBOL_ID_MAX - start_index, packets_to_send);

        BlockEncoder::add_packets(&mut blocks, encoder.repair_packets(start_index as u32, packets_created as u32), block_id);

        if packets_created < packets_to_send {
            BlockEncoder::add_packets(&mut blocks, encoder.repair_packets(0, (packets_to_send - packets_created) as u32), block_id);
        }

        return blocks;
    }

    /// Creates packets to transmit.
    pub fn generate_encoded_blocks(&self) -> Vec<EncodedBlock> {
        return BlockEncoder::encode_data(&self.config, &self.encoding_plan, &self.data, self.packet_size, self.block_id);
    }

    /// Gets information about payload required for decoding.
    pub fn get_block_info(&self) -> BlockInfo {
        return BlockInfo {
            payload_size: self.payload_size,
            padded_size: self.data.len(),
            config: self.config,
            block_id: self.block_id,
        };
    }
}
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{EncodingPacket, ObjectTransmissionInformation, SourceBlockEncoder};
use std::cmp;
use super::consts::*;
use rand::{thread_rng, Rng};

pub struct RaptorQEncoder {
    data_size: usize,
    packet_size: u16,
    block_encoders: Vec<BlockEncoder>,
}

impl RaptorQEncoder {
    pub fn new(data: &[u8], packet_size: u16) -> Result<RaptorQEncoder, RaptorQEncoderError> {
        let block_size = RAPTORQ_MAX_SYMBOLS_IN_BLOCK * packet_size as usize;

        let data_chunks: Vec<Vec<u8>> = data.chunks(block_size).map(|x| x.to_vec()).collect();

        // create block encoders
        let mut block_encoders: Vec<BlockEncoder> = Vec::new();
        for (i, data_chunk) in data_chunks.iter().enumerate() {
            match BlockEncoder::new(i as u32, packet_size, data_chunk.to_vec()) {
                Ok(block_encoder) => block_encoders.push(block_encoder),
                Err(error) => return Err(error),
            }
        }
        return Ok(RaptorQEncoder {
            data_size: data.len(),
            packet_size: packet_size,
            block_encoders: block_encoders,
        });
    }

    pub fn generate_encoded_blocks(&self) -> Vec<EncodedBlock> {
        let mut blocks: Vec<EncodedBlock> = Vec::new();

        for block_encoder in self.block_encoders.iter() {
            blocks.append(&mut block_encoder.generate_encoded_blocks());
        }

        return blocks;
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
    pub block_id: u32,
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

        let source_block_size_limit = RAPTORQ_MAX_SYMBOLS_IN_BLOCK * packet_size as usize;

        let max_data_size = source_block_size_limit;
        if data.len() > max_data_size as usize {
            return Err(RaptorQEncoderError::DataSizeTooLarge);
        }

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
    pub(crate) fn encode_data(config: &ObjectTransmissionInformation, data: &[u8], packet_size: u16, block_id: u32) -> Vec<EncodedBlock> {
        let encoder = SourceBlockEncoder::new2(0, config, data);
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
        return BlockEncoder::encode_data(&self.config, &self.data, self.packet_size, self.block_id);
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

#[cfg(test)]
use super::decoder::*;
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
    fn test_encoder_invalid_packet_size() {
        let packet_size: u16 = 1337;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);
        
        match BlockEncoder::new(0, packet_size, data.clone()) {
            Ok(_) => panic!("Should have failed to use packet_size {} with alignment {}", packet_size, ALIGNMENT),
            Err(error) => assert_eq!(error, RaptorQEncoderError::InvalidPacketSize),
        };
    }
    
    #[test]
    fn test_encoder_single_client() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);
        
        let encoder = match BlockEncoder::new(0, packet_size, data.clone()) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };
        let packets = encoder.generate_encoded_blocks();
        
        match BlockDecoder::decode_data(encoder.get_block_info(), packets) {
            Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
            Err(error) => panic!("Failed to decode data, err {}", error as u32),
        }
    }
    
    #[test]
    fn test_encoder_multiple_peers() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);
        
        let encoder = match BlockEncoder::new(0, packet_size, data.clone()) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };
        // pretend we have three different client streams
        let mut packets = encoder.generate_encoded_blocks();
        let mut packets_2 = encoder.generate_encoded_blocks();
        let mut packets_3 = encoder.generate_encoded_blocks();
        
        // lose 2/3 of each stream, to simulate receiving partial data from multiple clients
        let packets_per_client = data_size / (3 * packet_size as usize) + 1;
        packets.truncate(packets_per_client);
        packets_2.truncate(packets_per_client);
        packets_3.truncate(packets_per_client);
        
        // recombine into single stream
        packets.append(&mut packets_2);
        packets.append(&mut packets_3);
        
        // recover data
        match BlockDecoder::decode_data(encoder.get_block_info(), packets) {
            Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
            Err(error) => panic!("Failed to decode data, err {}", error as u32),
        }
    }
}

#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockEncoder,
};

/// Alignment of symbols in memory. 
const ALIGNMENT: u8 = 8;

/// A representation of a RaptorQEncoder
pub struct RaptorQEncoder {
    /// RaptorQ configuration object
    config: ObjectTransmissionInformation,
    /// Data to be encoded with the RaptorQ scheme (padded to a multiple of packet_size)
    data: Vec<u8>,
    /// Original size of data before padding.
    payload_size: usize,
    /// Encoded packet size. Also the symbol size used for RaptorQEncoder. 
    packet_size: u16,
}

#[derive(Debug)]
pub enum RaptorQEncoderError {
    InvalidPacketSize,
}

/// Information about the payload encoded by a RaptorQEncoder. Needs to be transmitted from the encoder to the decoder. 
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct PayloadInfo {
    /// Size of payload in data.
    pub payload_size: usize,
    // Actual size of data, including padding. 
    pub padded_size: usize,
    /// RaptorQ configuration object
    pub config: ObjectTransmissionInformation,
}

impl RaptorQEncoder {
    /// Creates a RaptorQEncoder with a given data payload and packet size
    /// TODO: fix internal assert that packet_size is a multiple of 8 (alignment). 
    pub fn new(mut data: Vec<u8>, packet_size: u16) -> Result<RaptorQEncoder, RaptorQEncoderError> {
        if packet_size % ALIGNMENT as u16 != 0 {
            return Err(RaptorQEncoderError::InvalidPacketSize);
        }

        let payload_size = data.len();

        // The rust RaptorQ library asserts data length to be a multiple of packet size, pad with zeros.
        if data.len() % packet_size as usize > 0 {
            data.resize(data.len() + (packet_size as usize - (data.len() % packet_size as usize)), 0);
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
        return Ok(RaptorQEncoder {
            config: ObjectTransmissionInformation::new(data.len() as u64, packet_size, 1, 1, ALIGNMENT),
            data: data, 
            payload_size: payload_size,
            packet_size: packet_size,
        })
    }

    /// Creates packets to transmit. 
    pub fn create_packets(&self, peer_index: u8) -> Vec<EncodingPacket> {
        let encoder = SourceBlockEncoder::new2(1, &self.config, &self.data);

        let length_in_packets = self.data.len()/self.packet_size as usize;

        // encoding symbol ids must be less than 2^24
        let mut start_symbol_id = (peer_index as usize * self.data.len()) % (1 << 24);
        if start_symbol_id > length_in_packets {
            start_symbol_id -= length_in_packets;
        }

        return encoder.repair_packets(start_symbol_id as u32, length_in_packets as u32)
    }

    /// Gets information about payload required for decoding.
    pub fn get_payload_info(&self) -> PayloadInfo {
        return PayloadInfo{payload_size: self.payload_size, padded_size: self.data.len(), config: self.config};
    }
}
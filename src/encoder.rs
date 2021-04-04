use raptorq::{EncodingPacket, ObjectTransmissionInformation, SourceBlockEncoder};
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

/// Alignment of symbols in memory in bytes.
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaptorQEncoderError {
    /// Packet size provided is not valid. 
    /// TODO: make errors more useful. 
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
    pub fn new(mut data: Vec<u8>, packet_size: u16) -> Result<RaptorQEncoder, RaptorQEncoderError> {
        if packet_size % ALIGNMENT as u16 != 0 {
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
        });
    }

    /// Creates packets to transmit.
    pub fn create_packets(&self, peer_index: u8) -> Vec<EncodingPacket> {
        let encoder = SourceBlockEncoder::new2(1, &self.config, &self.data);

        let length_in_packets = self.data.len() / self.packet_size as usize;

        // encoding symbol ids must be less than 2^24
        let mut start_symbol_id = (peer_index as usize * self.data.len()) % (1 << 24);
        if start_symbol_id > length_in_packets {
            start_symbol_id -= length_in_packets;
        }

        return encoder.repair_packets(start_symbol_id as u32, length_in_packets as u32);
    }

    /// Gets information about payload required for decoding.
    pub fn get_payload_info(&self) -> PayloadInfo {
        return PayloadInfo {
            payload_size: self.payload_size,
            padded_size: self.data.len(),
            config: self.config,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use raptorq::SourceBlockDecoder;

    fn gen_data(len: usize) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::with_capacity(len);
        for _ in 0..len {
            data.push(rand::thread_rng().gen());
        }
        return data;
    }

    fn decode_packets(
        config: &ObjectTransmissionInformation,
        packets: Vec<EncodingPacket>,
        source_block_length: u64,
    ) -> Option<Vec<u8>> {
        let mut decoder = SourceBlockDecoder::new2(1, &config, source_block_length);
        return decoder.decode(packets);
    }

    #[test]
    fn test_encoder_invalid_packet_size() {
        let packet_size: u16 = 1337;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);

        match RaptorQEncoder::new(data.clone(), packet_size) {
            Ok(_) => panic!("Should have failed to use packet_size {} with alignment {}", packet_size, ALIGNMENT),
            Err(error) => assert_eq!(error, RaptorQEncoderError::InvalidPacketSize),
        };
    }

    #[test]
    fn test_encoder_single_client_peer_0() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);

        let encoder = match RaptorQEncoder::new(data.clone(), packet_size) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };
        let packets = encoder.create_packets(0);

        let recovered_data = decode_packets(
            &encoder.get_payload_info().config,
            packets,
            encoder.get_payload_info().padded_size as u64,
        );

        assert_eq!(recovered_data.is_some(), true);

        let errors = recovered_data
            .unwrap()
            .iter()
            .zip(&data)
            .filter(|&(a, b)| a != b)
            .count();
        assert_eq!(errors, 0);
    }

    #[test]
    fn test_encoder_single_client_random_peer() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);

        let encoder = match RaptorQEncoder::new(data.clone(), packet_size) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };
        println!("Using random peer {}", data[0]);
        let packets = encoder.create_packets(data[0]);

        let recovered_data = decode_packets(
            &encoder.get_payload_info().config,
            packets,
            encoder.get_payload_info().padded_size as u64,
        );

        assert_eq!(recovered_data.is_some(), true);

        let errors = recovered_data
            .unwrap()
            .iter()
            .zip(&data)
            .filter(|&(a, b)| a != b)
            .count();
        assert_eq!(errors, 0);
    }

    #[test]
    fn test_encoder_multiple_peers() {
        let packet_size: u16 = 1280;
        let data_size: usize = 128 * 1024;
        let data = gen_data(data_size);

        let encoder = match RaptorQEncoder::new(data.clone(), packet_size) {
            Ok(succ) => succ,
            Err(error) => panic!("Failed to create encoder, error {}", error as u32),
        };
        // pretend we have three different client streams
        let mut packets = encoder.create_packets(110);
        let mut packets_2 = encoder.create_packets(13);
        let mut packets_3 = encoder.create_packets(255);

        // lose 2/3 of each stream, to simulate receiving partial data from multiple clients
        let packets_per_client = data_size / (3 * packet_size as usize) + 1;
        packets.truncate(packets_per_client);
        packets_2.truncate(packets_per_client);
        packets_3.truncate(packets_per_client);

        // recombine into single stream
        packets.append(&mut packets_2);
        packets.append(&mut packets_3);

        // recover data
        let recovered_data = decode_packets(
            &encoder.get_payload_info().config,
            packets,
            encoder.get_payload_info().padded_size as u64,
        );
        assert_eq!(recovered_data.is_some(), true);

        let errors = recovered_data
            .unwrap()
            .iter()
            .zip(&data)
            .filter(|&(a, b)| a != b)
            .count();
        assert_eq!(errors, 0);
    }
}

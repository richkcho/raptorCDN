use rand::Rng;
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder, SourceBlockEncoder,
};

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    return data;
}

fn get_encoded_packets(config: &ObjectTransmissionInformation, data: &[u8], start_repair_symbol_id: u32, packets: u32) -> Vec<EncodingPacket> {
    let encoder = SourceBlockEncoder::new2(1, config, data);

    return encoder.repair_packets(start_repair_symbol_id, packets);
}

fn decode_packets(config: &ObjectTransmissionInformation, packets: Vec<EncodingPacket>, source_block_length: u64) -> Option<Vec<u8>> {
    let mut decoder = SourceBlockDecoder::new2(1, &config, source_block_length);

    return decoder.decode(packets);
}

fn main() {
    // RaptorQ is applied to source blocks independently. Assume 1 source block for now.
    // data_size % packet_size (symbol_size) should be zero, otherwise library panics. 
    let packet_size: u16 = 1024;
    let data_size: usize = 128 * 1024;

    let data = gen_data(data_size);

    /*
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
     * In this case, we are transferring 1 source block with all of the data. Sub-block count is chosen to support multiple encoders.
     * Actually, I have no idea what this sub-block stuff is in relation to the library. 
     */
    let config = ObjectTransmissionInformation::new(data_size as u64, packet_size, 1, 32, 8);
    
    // Simulate creating encoded packets from different clients
    // We want to select packets that haven't been included by another client. (by index)
    // One way to ensure this is to ask clients to send packets starting from offset of filesize * n for client n. 
    let mut packets = get_encoded_packets(&config, &data, 0, 64);
    let mut packets_2 = get_encoded_packets(&config, &data, (data_size/packet_size as usize) as u32, 64);

    packets.append(&mut packets_2);

    let recovered_data = decode_packets(&config, packets, data_size as u64);

    if recovered_data.is_some() {
        println!("Data recovered.");
        let matching = recovered_data.unwrap().iter().zip(&data).filter(|&(a, b)| a != b).count();
        println!("Mismatched data count {}", matching);
    } else {
        println!("Data not recovered.");
    }
}

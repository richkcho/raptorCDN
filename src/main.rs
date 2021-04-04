use rand::Rng;
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder, SourceBlockEncoder,
};
mod encoder;

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    return data;
}

fn decode_packets(config: &ObjectTransmissionInformation, packets: Vec<EncodingPacket>, source_block_length: u64) -> Option<Vec<u8>> {
    let mut decoder = SourceBlockDecoder::new2(1, &config, source_block_length);

    return decoder.decode(packets);
}

fn main() {
    let packet_size: u16 = 1280;
    let data_size: usize = 128 * 1024;

    let data = gen_data(data_size);

    let encoder = encoder::RaptorQEncoder::new(data.clone(), packet_size);
    
    // pretend we have three different client streams
    let mut packets = encoder.create_packets(0);
    let mut packets_2 = encoder.create_packets(1);
    let mut packets_3 = encoder.create_packets(2);
    
    // lose 2/3 of each stream, to simulate receiving partial data from multiple clients
    let packets_per_client = data_size/(3 * packet_size as usize) + 1;
    packets.truncate(packets_per_client);
    packets_2.truncate(packets_per_client);
    packets_3.truncate(packets_per_client);

    // recombine into single stream
    packets.append(&mut packets_2);
    packets.append(&mut packets_3);

    // recover data
    let recovered_data = decode_packets(&encoder.get_payload_info().config, packets, encoder.get_payload_info().padded_size as u64);

    if recovered_data.is_some() {
        println!("Data recovered.");
        let matching = recovered_data.unwrap().iter().zip(&data).filter(|&(a, b)| a != b).count();
        println!("Mismatched data count {}", matching);
    } else {
        println!("Data not recovered.");
    }
}

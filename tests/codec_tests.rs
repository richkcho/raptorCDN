use std::collections::HashSet;
use rand::Rng;
use raptor_cdn_lib::codec::consts::*;
use raptor_cdn_lib::codec::decoder::*;
use raptor_cdn_lib::codec::encoder::*;

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    data
}

fn arr_eq(data1: &[u8], data2: &[u8]) -> bool {
    data1.iter().zip(data2.iter()).all(|(a, b)| a == b)
}

#[test]
fn test_block_encoder_invalid_packet_size() {
    let packet_size: u16 = 1337;
    let data_size: usize = packet_size as usize * MAX_SYMBOLS_IN_BLOCK as usize;
    let data = gen_data(data_size);

    match BlockEncoder::new(0, packet_size, data.clone(), None) {
        Ok(_) => panic!(
            "Should have failed to use packet_size {} with alignment {}",
            packet_size, ALIGNMENT
        ),
        Err(_) => (),
    };
}

#[test]
fn test_block_encoder_single_peer() {
    let packet_size: u16 = 1280;
    let data_size: usize = packet_size as usize * MAX_SYMBOLS_IN_BLOCK as usize;
    let data = gen_data(data_size);

    let encoder = match BlockEncoder::new(0, packet_size, data.clone(), None) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder: {}", error),
    };
    let blocks = encoder.generate_encoded_blocks();

    match BlockDecoder::decode_data(&encoder.get_block_info(), blocks) {
        Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
        Err(error) => panic!("Failed to decode data: {}", error),
    }
}

#[test]
fn test_block_encoder_multiple_peers() {
    let packet_size: u16 = 1280;
    let data_size: usize = packet_size as usize * MAX_SYMBOLS_IN_BLOCK as usize;
    let data = gen_data(data_size);

    let encoder = match BlockEncoder::new(0, packet_size, data.clone(), None) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder: {}", error),
    };
    // pretend we have three different client streams
    let mut blocks = encoder.generate_encoded_blocks();
    let mut blocks_2 = encoder.generate_encoded_blocks();
    let mut blocks_3 = encoder.generate_encoded_blocks();

    // lose 2/3 of each stream, to simulate receiving partial data from multiple clients
    let packets_per_client = data_size / (3 * packet_size as usize) + 1;
    blocks.truncate(packets_per_client);
    blocks_2.truncate(packets_per_client);
    blocks_3.truncate(packets_per_client);

    // recombine into single stream
    blocks.append(&mut blocks_2);
    blocks.append(&mut blocks_3);

    // recover data
    match BlockDecoder::decode_data(&encoder.get_block_info(), blocks) {
        Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
        Err(error) => panic!("Failed to decode data: {}", error),
    }
}

#[test]
fn test_block_decode_single_client() {
    let packet_size: u16 = 1280;
    let data_size: usize = packet_size as usize * MAX_SYMBOLS_IN_BLOCK as usize;
    let data = gen_data(data_size);

    let encoder = match BlockEncoder::new(0, packet_size, data.clone(), None) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder: {}", error),
    };

    let blocks = encoder.generate_encoded_blocks();

    let decoder = match BlockDecoder::new(encoder.get_block_info()) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder: {}", error),
    };

    match decoder.decode_blocks(blocks) {
        Ok(recovered_data) => assert_eq!(arr_eq(&recovered_data, &data), true),
        Err(error) => panic!("Failed to decode data: {}", error),
    }
}

#[test]
fn test_encode_decode_e2e() {
    let packet_size: u16 = 512;
    let data_size: usize = 123456;

    let data = gen_data(data_size);

    let mut plans = load_encoding_plans(".encoding_plan_cache/").unwrap();
    let plan_keys: HashSet<u16> = plans.keys().copied().collect();

    let encoder = match RaptorQEncoder::new(packet_size, &data, Some(&mut plans)) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder, error {}", error),
    };

    plans.retain(|k,_| !plan_keys.contains(k));
    save_encoding_plans(".encoding_plan_cache/", &plans).unwrap();

    let mut decoder = match RaptorQDecoder::new(encoder.get_block_info_vec()) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create decoder, error {}", error),
    };

    let blocks_total = encoder.generate_encoded_blocks();

    decoder.consume_blocks(blocks_total);
    let result = decoder.decode_blocks();

    let recovered_data = match result {
        Ok(data) => data,
        Err(error) => panic!("Failed to decode, error {}", error),
    };

    assert_eq!(data, recovered_data);
}
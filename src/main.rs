mod codec;
use num_format::{Locale, ToFormattedString};
use rand::Rng;
use std::time::Instant;

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    data
}

fn main() {
    println!("I do nothing for now.");
    let packet_size: u16 = 1280;
    let num_blocks: usize = 30;
    let data_size: usize = codec::consts::MAX_SYMBOLS_IN_BLOCK * packet_size as usize * num_blocks;
    println!("data size {}", data_size.to_formatted_string(&Locale::en));
    // for this test to work, we expect NO PADDING!
    assert_eq!(data_size % packet_size as usize, 0);
    println!("Generating data...");
    let data = gen_data(data_size);

    println!("Creating encoder...");
    let mut now = Instant::now();
    let encoder = match codec::encoder::RaptorQEncoder::new(packet_size, &data) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder, error {}", error as u32),
    };
    println!("Created encoder in {} ms", now.elapsed().as_millis());
    println!("Creating decoder...");
    now = Instant::now();
    let mut decoder = match codec::decoder::RaptorQDecoder::new(encoder.get_block_info_vec()) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create decoder, error {}", error as u32),
    };
    println!("Created decoder in {} ms", now.elapsed().as_millis());

    println!("Generating encoded data...");
    now = Instant::now();
    let blocks_total = encoder.generate_encoded_blocks();
    println!("Generated encoded data in {} ms", now.elapsed().as_millis());

    println!("Decoding encoded data...");
    now = Instant::now();
    decoder.consume_blocks(blocks_total);
    let result = decoder.decode_blocks();
    println!("Decoded data in {} ms", now.elapsed().as_millis());

    let recovered_data = match result {
        Ok(data) => data,
        Err(error) => panic!("Failed to decode, error {}", error as u32),
    };

    assert_eq!(data, recovered_data);
    println!("Everything worked!");
}

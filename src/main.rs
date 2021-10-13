mod codec;
use std::time::{Duration, Instant};
use rand::Rng;
use num_format::{Locale, ToFormattedString};

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    return data;
}

fn main() {
    println!("I do nothing for now.");
    let packet_size: u16 = 16384;
    let num_blocks: usize = 3;
    let data_size: usize = codec::consts::MAX_SYMBOLS_IN_BLOCK * packet_size as usize * num_blocks;
    
    println!("data size {}", data_size.to_formatted_string(&Locale::en));
    
    // for this test to work, we expect NO PADDING!
    assert_eq!(data_size % packet_size as usize, 0);
    
    println!("Generating data...");
    let data = gen_data(data_size);

    println!("Creating encoder...");
    let encoder = match codec::encoder::RaptorQEncoder::new(packet_size, &data) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder, error {}", error as u32),
    };
    
    println!("Generating encoded data...");
    let now = Instant::now();
    let mut blocks_total = encoder.generate_encoded_blocks();
    println!("Duration {}", now.elapsed().as_millis());
}
mod codec;
use num_format::{Locale, ToFormattedString};
use rand::Rng;
use std::time::Instant;
use std::collections::HashSet;

fn gen_data(len: usize) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(rand::thread_rng().gen());
    }
    data
}

fn main() {
    println!("I do nothing for now.");
    let packet_size: u16 = 512;
    let data_size: usize = 123456789;


    println!("data size {}", data_size.to_formatted_string(&Locale::en));
    println!("Generating data...");
    let mut now = Instant::now();
    let data = gen_data(data_size);
    println!("Generated data in {} ms", now.elapsed().as_millis());

    println!("Loading cached SourceBlockEncodingPlans");
    now = Instant::now();
    let mut plans = codec::encoder::load_encoding_plans(".encoding_plan_cache/").unwrap();
    let plan_keys: HashSet<u16> = plans.keys().copied().collect();
    println!("Loaded {} cached SourceBlockEncodingPlans in {} ms", plans.keys().len(), now.elapsed().as_millis());
    

    println!("Creating encoder...");
    let encoder = match codec::encoder::RaptorQEncoder::new(packet_size, &data, Some(&mut plans)) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create encoder, error {}", error),
    };
    println!("Created encoder in {} ms", now.elapsed().as_millis());

    println!("Saving updated SourceBlockEncodingPlans");
    now = Instant::now();
    plans.retain(|k,_| !plan_keys.contains(k));
    codec::encoder::save_encoding_plans(".encoding_plan_cache/", &plans).unwrap();
    println!("Saved {} cached SourceBlockEncodingPlans in {} ms", plans.keys().len(), now.elapsed().as_millis());

    println!("Creating decoder...");
    now = Instant::now();
    let mut decoder = match codec::decoder::RaptorQDecoder::new(encoder.get_block_info_vec()) {
        Ok(succ) => succ,
        Err(error) => panic!("Failed to create decoder, error {}", error),
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
        Err(error) => panic!("Failed to decode, error {}", error),
    };

    assert_eq!(data, recovered_data);
    println!("Everything worked!");
}

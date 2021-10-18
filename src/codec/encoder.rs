use super::consts::*;
use anyhow;
use anyhow::Result;
use rand::{seq::SliceRandom, thread_rng, Rng};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockEncoder, SourceBlockEncodingPlan,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::cmp;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedEncodingPlan {
    source_symbol_count: u16,
    plan: SourceBlockEncodingPlan,
}

fn load_cached_encoding_plan(path: &path::PathBuf) -> Result<CachedEncodingPlan> {
    let json_data = fs::read_to_string(path)?;

    Ok(serde_json::from_str::<CachedEncodingPlan>(&json_data)?)
}

fn load_cached_encoding_plans(dir: &str) -> Result<Vec<CachedEncodingPlan>> {
    let plan_files: Vec<path::PathBuf> = fs::read_dir(dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?
        .into_iter()
        .filter(|p| {
            p.is_file() && p.extension().and_then(std::ffi::OsStr::to_str) == Some("json")
        })
        .collect();

    plan_files.iter().map(load_cached_encoding_plan).collect()
}

pub fn load_encoding_plans(dir: &str) -> Result<HashMap<u16, SourceBlockEncodingPlan>> {
    let mut hashmap: HashMap<u16, SourceBlockEncodingPlan> = HashMap::new();

    for cached_plan in load_cached_encoding_plans(dir)? {
        hashmap.insert(cached_plan.source_symbol_count, cached_plan.plan);
    }

    Ok(hashmap)
}

fn save_encoding_plan(dir: &str, cached_plan: CachedEncodingPlan) -> Result<()> {
    let file_path = path::Path::new(dir).join(format!("plan_{}.json", cached_plan.source_symbol_count));

    Ok(fs::write(file_path, serde_json::to_string(&cached_plan)?)?)
}

pub fn save_encoding_plans(dir: &str, plans: HashMap<u16, SourceBlockEncodingPlan>) -> Result<()> {
    let cached_plans: Vec<CachedEncodingPlan> = plans.iter().map(|(k, v)| CachedEncodingPlan {source_symbol_count: *k, plan: v.clone()}).collect();

    for cached_plan in cached_plans {
        save_encoding_plan(dir, cached_plan)?;
    }

    Ok(())
}

struct EncodingPlanCache {
    lock: std::sync::RwLock<usize>,
    plans: HashMap<u16, SourceBlockEncodingPlan>,
}

pub struct RaptorQEncoder {
    block_encoders: Vec<BlockEncoder>,
    encoding_plan_cache: EncodingPlanCache,
}

/// RaptorQ data encoder.
impl RaptorQEncoder {
    pub fn new(packet_size: u16, data: &[u8], encoding_plans: HashMap<u16, SourceBlockEncodingPlan>) -> Result<RaptorQEncoder, RaptorQEncoderError> {
        let block_size = MAX_SYMBOLS_IN_BLOCK as usize * packet_size as usize;

        let data_chunks: Vec<Vec<u8>> = data.chunks(block_size).map(|x| x.to_vec()).collect();

        let mut encoding_plan_cache = EncodingPlanCache {lock:std::sync::RwLock::new(0), plans: encoding_plans};

        let encoder_results: Result<Vec<BlockEncoder>, RaptorQEncoderError> = data_chunks.iter().enumerate().map(|(i, chunk)| (i, chunk, &encoding_plan_cache))
            .par_bridge()
            .map(|(i, data_chunk, mut cache_ref)| BlockEncoder::new(i as u32, packet_size, data_chunk.to_vec(), &cache_ref))
            .collect();


        match encoder_results {
            Ok(block_encoders) => Ok(RaptorQEncoder { block_encoders, encoding_plan_cache}),
            Err(error) => Err(error),
        }
    }

    pub fn generate_encoded_blocks(&self) -> Vec<EncodedBlock> {
        // shuffle blocks so multiple encoders don't all send blocks in the same order. Transposing would in theory help here, but I don't know how costly that is.
        let mut encoded_blocks: Vec<Vec<EncodedBlock>> = self
            .block_encoders
            .par_iter()
            .map(|encoder| encoder.generate_encoded_blocks())
            .collect();
        encoded_blocks.shuffle(&mut thread_rng());
        encoded_blocks.into_iter().flatten().collect()
    }

    pub fn get_block_info_vec(&self) -> Vec<BlockInfo> {
        self.block_encoders
            .par_iter()
            .map(|x| x.get_block_info())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaptorQEncoderError {
    /// Packet size provided is not valid.
    /// TODO: make errors more useful.
    InvalidPacketSize,
    DataSizeTooLarge,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct EncodedBlock {
    /// Index of this block in overall payload
    pub block_id: u32,
    /// raptorq packet
    pub data: EncodingPacket,
}

/// Information about the payload encoded by a BlockEncoder. Needs to be transmitted from the encoder to the decoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn new(
        block_id: u32,
        packet_size: u16,
        mut data: Vec<u8>,
        encoding_plan_cache: &mut EncodingPlanCache
    ) -> Result<BlockEncoder, RaptorQEncoderError> {
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

        let num_symbols = (data.len() / packet_size as usize) as u16;

        if num_symbols > MAX_SYMBOLS_IN_BLOCK {
            return Err(RaptorQEncoderError::DataSizeTooLarge);
        }

        let (update_cache, encoding_plan) = match (encoding_plan_cache.lock.read().unwrap(), encoding_plan_cache.plans.get(&num_symbols)) {
            (_, Some(plan)) => (false, plan.clone()),
            (_, None) => (true, SourceBlockEncodingPlan::generate(num_symbols)),
        };
        
        if update_cache {
            encoding_plan_cache.lock.write().unwrap();
            encoding_plan_cache.plans.insert(num_symbols, encoding_plan.clone());
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
        Ok(BlockEncoder {
            config: ObjectTransmissionInformation::new(
                data.len() as u64,
                packet_size,
                1,
                1,
                ALIGNMENT,
            ),
            data,
            payload_size,
            packet_size,
            block_id,
            encoding_plan,
        })
    }

    fn add_packets(
        blocks: &mut Vec<EncodedBlock>,
        mut packets: Vec<EncodingPacket>,
        block_id: u32,
    ) {
        while match packets.pop() {
            None => false,
            Some(packet) => {
                blocks.push(EncodedBlock {
                    block_id,
                    data: packet,
                });
                true
            }
        } {}
    }

    /// static method for encoding data
    pub fn encode_data(
        config: &ObjectTransmissionInformation,
        plan: &SourceBlockEncodingPlan,
        data: &[u8],
        packet_size: u16,
        block_id: u32,
    ) -> Vec<EncodedBlock> {
        let encoder = SourceBlockEncoder::with_encoding_plan2(0, config, data, plan);
        let packets_to_send = data.len() / packet_size as usize;
        let mut blocks: Vec<EncodedBlock> = Vec::new();

        let start_index = thread_rng().gen_range(0..RAPTORQ_ENCODING_SYMBOL_ID_MAX);

        let packets_created = cmp::min(
            RAPTORQ_ENCODING_SYMBOL_ID_MAX - start_index,
            packets_to_send,
        );

        BlockEncoder::add_packets(
            &mut blocks,
            encoder.repair_packets(start_index as u32, packets_created as u32),
            block_id,
        );
        if packets_created < packets_to_send {
            BlockEncoder::add_packets(
                &mut blocks,
                encoder.repair_packets(0, (packets_to_send - packets_created) as u32),
                block_id,
            );
        }

        blocks
    }

    /// Creates packets to transmit.
    pub fn generate_encoded_blocks(&self) -> Vec<EncodedBlock> {
        BlockEncoder::encode_data(
            &self.config,
            &self.encoding_plan,
            &self.data,
            self.packet_size,
            self.block_id,
        )
    }

    /// Gets information about payload required for decoding.
    pub fn get_block_info(&self) -> BlockInfo {
        BlockInfo {
            payload_size: self.payload_size,
            padded_size: self.data.len(),
            config: self.config,
            block_id: self.block_id,
        }
    }
}

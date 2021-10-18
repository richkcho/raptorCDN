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
use std::io::prelude::*;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedEncodingPlan {
    source_symbol_count: u16,
    plan: SourceBlockEncodingPlan,
}

fn load_cached_encoding_plan(path: &path::PathBuf) -> Result<CachedEncodingPlan> {
    let mut zlib_decoder = ZlibDecoder::new(fs::File::open(path)?);
    let mut json_data = String::new();
    zlib_decoder.read_to_string(&mut json_data)?;

    Ok(serde_json::from_str::<CachedEncodingPlan>(&json_data)?)
}

fn load_cached_encoding_plans(dir: &str) -> Result<Vec<CachedEncodingPlan>> {
    let plan_files: Vec<path::PathBuf> = fs::read_dir(dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?
        .into_iter()
        .filter(|p| p.is_file() && p.extension().and_then(std::ffi::OsStr::to_str) == Some("json-zip"))
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

fn save_encoding_plan(dir: &str, cached_plan: &CachedEncodingPlan) -> Result<()> {
    let file_path =
        path::Path::new(dir).join(format!("plan_{}.json-zip", cached_plan.source_symbol_count));

    let mut zlib_encoder = ZlibEncoder::new(fs::File::create(file_path)?, Compression::default());
    zlib_encoder.write_all(serde_json::to_string(&cached_plan)?.as_bytes())?;
    zlib_encoder.finish()?;

    Ok(())
}

pub fn save_encoding_plans(dir: &str, plans: &HashMap<u16, SourceBlockEncodingPlan>) -> Result<()> {
    let cached_plans: Vec<CachedEncodingPlan> = plans
        .iter()
        .map(|(k, v)| CachedEncodingPlan {
            source_symbol_count: *k,
            plan: v.clone(),
        })
        .collect();

    for cached_plan in cached_plans {
        save_encoding_plan(dir, &cached_plan)?;
    }

    Ok(())
}

pub struct RaptorQEncoder {
    block_encoders: Vec<BlockEncoder>,
}

/// RaptorQ data encoder.
impl RaptorQEncoder {
    pub fn new(
        packet_size: u16,
        data: &[u8],
        encoding_plan_cache: Option<&mut HashMap<u16, SourceBlockEncodingPlan>>,
    ) -> Result<RaptorQEncoder> {
        let block_size = MAX_SYMBOLS_IN_BLOCK as usize * packet_size as usize;

        let data_chunks: Vec<Vec<u8>> = data.chunks(block_size).map(|x| x.to_vec()).collect();

        let block_encoders: Vec<BlockEncoder> = data_chunks
            .par_iter()
            .enumerate()
            .map(|(i, data_chunk)| {
                BlockEncoder::new(
                    i as u32,
                    packet_size,
                    data_chunk.to_vec(),
                    match &encoding_plan_cache {
                        Some(plan) => Some(&*plan),
                        None => None,
                    },
                )
            })
            .collect::<Result<Vec<BlockEncoder>>>()?;

        // if cache was provided, update it. 
        match encoding_plan_cache {
            Some(cache) => {
                let uncached_values = block_encoders
                    .iter()
                    .map(|encoder| {
                        (
                            (encoder.data.len() / encoder.packet_size as usize) as u16,
                            encoder.encoding_plan.clone(),
                        )
                    })
                    .filter(|(syms, _)| !cache.contains_key(&syms))
                    .collect::<Vec<(u16, SourceBlockEncodingPlan)>>();
                for uncached_value in uncached_values {
                    cache.insert(uncached_value.0, uncached_value.1);
                }
            }
            None => (),
        }

        Ok(RaptorQEncoder { block_encoders })
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
        encoding_plan_cache: Option<&HashMap<u16, SourceBlockEncodingPlan>>,
    ) -> Result<BlockEncoder> {
        if packet_size % ALIGNMENT as u16 != 0 || packet_size < MIN_PACKET_SIZE {
            return Err(anyhow::anyhow!(
                "BlockEncoder error: bad packet size {}",
                packet_size
            ));
        }

        let payload_size = data.len();

        // The rust RaptorQ library asserts data length to be a multiple of packet size, pad with zeros.
        if data.len() % packet_size as usize > 0 {
            data.resize(
                data.len() + (packet_size as usize - (data.len() % packet_size as usize)),
                0,
            );
        }

        assert_eq!(data.len() % packet_size as usize, 0);

        let num_symbols = (data.len() / packet_size as usize) as u16;
        if num_symbols > MAX_SYMBOLS_IN_BLOCK {
            return Err(anyhow::anyhow!(
                "BlockEncoder error: data length too large: {}",
                payload_size
            ));
        }

        let encoding_plan = match encoding_plan_cache {
            Some(cache) => match cache.get(&num_symbols) {
                Some(plan) => plan.clone(),
                None => SourceBlockEncodingPlan::generate(num_symbols),
            },
            None => SourceBlockEncodingPlan::generate(num_symbols),
        };

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

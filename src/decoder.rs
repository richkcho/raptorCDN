#[cfg(feature = "serde_support")]

mod encoder;
use encoder::PayloadInfo;
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder,
};


/// A representation of a RaptorQEncoder
pub struct RaptorQDecoder {
    /// RaptorQ configuration object
    config: ObjectTransmissionInformation,
    /// Packets to decode
    packets: Vec<EncodingPacket>,
}
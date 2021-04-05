#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, SourceBlockDecoder,
};

use super::encoder;

/// A representation of a RaptorQEncoder
pub struct RaptorQDecoder {
    /// RaptorQ configuration object
    config: ObjectTransmissionInformation,
    /// Packets to decode
    packets: Vec<EncodingPacket>,
}
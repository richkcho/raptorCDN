/*
 * Constants defined in the RPC spec are prefixed with RAPTORQ_
 * Other constants are defined by the encoder implementation.
 */
// source block number limit as defined in the raptorq spec, but a little less.
pub const RAPTORQ_ENCODING_SYMBOL_ID_MAX: usize = 1 << 23;

/// Maximum symbols we allow in a block. This should be less than 56403, which is the maximum symbol per block allowed by raptorq. 
#[cfg(debug_assertions)]
pub const MAX_SYMBOLS_IN_BLOCK: u16 = 128;

#[cfg(not(debug_assertions))]
pub const MAX_SYMBOLS_IN_BLOCK: u16 = 1024;

/// Alignment of symbols in memory in bytes.
pub const ALIGNMENT: u8 = 8;

// We enforce a minimum and maximum packet size for our encoder - not specified in RFC, just 4 fun.
pub const MIN_PACKET_SIZE: u16 = 512;
pub const MAX_PACKET_SIZE: u16 = 8192;
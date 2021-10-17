/*
 * Constants defined in the RPC spec are prefixed with RAPTORQ_
 * Other constants are defined by the encoder implementation. 
 */
// source block number limit as defined in the raptorq spec, but a little less. 
pub const RAPTORQ_ENCODING_SYMBOL_ID_MAX: usize = 1 << 23;

/// Maximum symbols we allow in a block. This should be less than 56403, which is the maximum symbol per block allowed by raptorq. 
pub const MAX_SYMBOLS_IN_BLOCK: usize = 1024;

/// Alignment of symbols in memory in bytes.
pub const ALIGNMENT: u8 = 8;

// We enforce a minimum packet size for our encoder - not specified in RFC, but it makes code easier. 
pub const MIN_PACKET_SIZE: u16 = 512;
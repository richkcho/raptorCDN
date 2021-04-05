const fn const_min(a: usize, b: usize) -> usize {
    [a, b][(a > b) as usize]
}

/*
 * Constants defined in the RPC spec are prefixed with RAPTORQ_
 * Other constants are defined by the encoder implementation. 
 */
// source block number limit as defined in the raptorq spec.
pub const RAPTORQ_ENCODING_SYMBOL_ID_MAX: usize = 1 << 24;

/// Maximum symbols allowed to be in a block in the raptorq spec. 
pub const RAPTORQ_MAX_SYMBOLS_IN_BLOCK: usize = 56403;

/// Alignment of symbols in memory in bytes.
pub const ALIGNMENT: u8 = 8;

// We enforce a minimum packet size for our encoder - not specified in RFC, but it makes code easier. 
pub const MIN_PACKET_SIZE: u16 = 512;
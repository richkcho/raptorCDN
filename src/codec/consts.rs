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

/// How many peers for which we guarantee disjoint raptorq blocks. Should be a small divisor of RAPTORQ_ENCODING_SYMBOL_ID_MAX. 
pub const PEER_SPACE_SIZE: usize = 256;

/// Effective source block symbol count limit given peer partitioning. 
/// This relies on two factors: the symbol limit in a specific block as defined in raptorq, and the partitioning of the encoding symbol id space into PEER_SPACE_SIZE pieces. 
pub const SOURCE_BLOCK_SYMBOL_COUNT_LIMIT: usize = const_min(RAPTORQ_ENCODING_SYMBOL_ID_MAX/PEER_SPACE_SIZE, RAPTORQ_MAX_SYMBOLS_IN_BLOCK);

// We enforce a minimum packet size for our encoder - not specified in RFC, but it makes code easier. 
pub const MIN_PACKET_SIZE: u16 = 512;
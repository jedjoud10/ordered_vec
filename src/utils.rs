// An index pair containing the actual index and the version
pub struct IndexPair {
    // First 32 bits
    pub index: u32,

    // Last 32 bits
    pub version: u32,
}

impl IndexPair {
    // New
    pub fn new(index: usize, version: u32) -> Self {
        Self {
            index: index as u32,
            version,
        }
    }
}
 
// Convert an index and version to a u64 ID
pub fn to_id(pair: IndexPair) -> u64 {
    // We do the bit shifting magic
    let mut id = pair.index as u64;
    id |= (pair.version << 32) as u64;
    id
}
// Convert a u64 ID to an index and version
pub fn from_id(id: u64) -> IndexPair {
    // We do the bit shifting magic
    let index = ((id << 32) >> 32) as u32;
    let version = (id >> 32) as u32;
    IndexPair { index, version }
}
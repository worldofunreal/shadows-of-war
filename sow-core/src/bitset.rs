use serde::{Deserialize, Serialize};

/// A highly-optimized, flat bitset for tracking boolean state across a large ID space (like map tiles).
/// It stores bits sequentially in a flat `Vec<u64>` to eliminate pointer chasing and cache misses.
///
/// For network efficiency via Serde, `serialize` and `deserialize` convert this dense
/// array into a sparse `Vec<u32>` payload, sending only the active indices over the wire.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenseBitSet {
    /// Each u64 holds 64 bits. Total length = (max_capacity + 63) / 64.
    pub blocks: Vec<u64>,
}

impl DenseBitSet {
    /// Creates an empty BitSet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bit at `idx` to 1. Returns `true` if it was not already set.
    #[inline]
    pub fn insert(&mut self, idx: u32) -> bool {
        let idx = idx as usize;
        let block = idx / 64;
        let bit = idx % 64;

        if block >= self.blocks.len() {
            self.blocks.resize(block + 1, 0);
        }

        let old = self.blocks[block];
        let mask = 1 << bit;
        self.blocks[block] = old | mask;
        (old & mask) == 0
    }

    /// Clears the bit at `idx` to 0. Returns `true` if it was previously set.
    #[inline]
    pub fn remove(&mut self, idx: u32) -> bool {
        let idx = idx as usize;
        let block = idx / 64;
        let bit = idx % 64;

        if block < self.blocks.len() {
            let old = self.blocks[block];
            let mask = 1 << bit;
            if (old & mask) != 0 {
                self.blocks[block] = old & !mask;
                return true;
            }
        }
        false
    }

    /// Checks if the bit at `idx` is set.
    #[inline]
    pub fn contains(&self, idx: u32) -> bool {
        let idx = idx as usize;
        let block = idx / 64;
        if block < self.blocks.len() {
            let bit = idx % 64;
            (self.blocks[block] & (1 << bit)) != 0
        } else {
            false
        }
    }

    /// Returns an iterator over all set indices.
    /// Iteration is absolutely deterministic and spatially ordered (from index 0 to max).
    pub fn ones(&self) -> impl Iterator<Item = u32> + '_ {
        self.blocks.iter().enumerate().flat_map(|(b_idx, &block)| {
            let mut val = block;
            let offset = 0;
            std::iter::from_fn(move || {
                if val == 0 {
                    None
                } else {
                    let trailing = val.trailing_zeros();
                    val &= !(1 << trailing); // clear the lowest set bit
                    let global_idx = (b_idx * 64 + offset + trailing as usize) as u32;
                    Some(global_idx)
                }
            })
        })
    }

    /// Collect at most `limit` set indices, starting at `start_idx` and
    /// wrapping once through the bitset. The result stays deterministic while
    /// avoiding a full materialization when callers only need a small sample.
    pub fn sample_ones(&self, start_idx: u32, limit: usize, out: &mut Vec<u32>) {
        out.clear();
        if limit == 0 || self.blocks.is_empty() {
            return;
        }

        let block_count = self.blocks.len();
        let start_block = (start_idx as usize / 64) % block_count;
        let start_bit = start_idx as usize % 64;

        for offset in 0..block_count {
            let block_idx = (start_block + offset) % block_count;
            let mut bits = self.blocks[block_idx];
            if offset == 0 && start_bit > 0 {
                bits &= u64::MAX << start_bit;
            }
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                out.push((block_idx * 64 + bit) as u32);
                if out.len() == limit {
                    return;
                }
                bits &= bits - 1;
            }
        }

        if start_bit > 0 && out.len() < limit {
            let mut bits = self.blocks[start_block] & ((1u64 << start_bit) - 1);
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                out.push((start_block * 64 + bit) as u32);
                if out.len() == limit {
                    return;
                }
                bits &= bits - 1;
            }
        }
    }

    /// Return the first set index at or after `start_idx`, wrapping once.
    pub fn first_one_from(&self, start_idx: u32) -> Option<u32> {
        if self.blocks.is_empty() {
            return None;
        }

        let block_count = self.blocks.len();
        let start_block = (start_idx as usize / 64) % block_count;
        let start_bit = start_idx as usize % 64;
        for offset in 0..block_count {
            let block_idx = (start_block + offset) % block_count;
            let mut bits = self.blocks[block_idx];
            if offset == 0 && start_bit > 0 {
                bits &= u64::MAX << start_bit;
            }
            if bits != 0 {
                return Some((block_idx * 64 + bits.trailing_zeros() as usize) as u32);
            }
        }

        if start_bit > 0 {
            let bits = self.blocks[start_block] & ((1u64 << start_bit) - 1);
            if bits != 0 {
                return Some((start_block * 64 + bits.trailing_zeros() as usize) as u32);
            }
        }
        None
    }

    /// Counts total set bits.
    pub fn count_ones(&self) -> usize {
        self.blocks.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// Returns true if no bits are set.
    pub fn is_empty(&self) -> bool {
        self.blocks.iter().all(|&b| b == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::DenseBitSet;

    #[test]
    fn bounded_sampling_wraps_without_exceeding_limit() {
        let mut bits = DenseBitSet::new();
        bits.insert(1);
        bits.insert(65);
        bits.insert(130);

        let mut out = Vec::new();
        bits.sample_ones(65, 2, &mut out);
        assert_eq!(out, [65, 130]);

        bits.sample_ones(130, 3, &mut out);
        assert_eq!(out, [130, 1, 65]);

        bits.sample_ones(0, 1, &mut out);
        assert_eq!(out, [1]);
    }

    #[test]
    fn first_one_from_wraps_and_empty_is_none() {
        let mut bits = DenseBitSet::new();
        bits.insert(9);
        bits.insert(70);
        assert_eq!(bits.first_one_from(70), Some(70));
        assert_eq!(bits.first_one_from(71), Some(9));
        assert_eq!(DenseBitSet::new().first_one_from(0), None);
    }
}

// Custom Serialize: send only active indices (Sparse)
impl Serialize for DenseBitSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as a Vec<u32> to keep network payload small
        let indices: Vec<u32> = self.ones().collect();
        indices.serialize(serializer)
    }
}

// Custom Deserialize: reconstruct dense bitset from sparse indices
impl<'de> Deserialize<'de> for DenseBitSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let indices = Vec::<u32>::deserialize(deserializer)?;
        let mut bitset = DenseBitSet::new();
        // Presize if possible based on max index to avoid resizing
        if let Some(&max) = indices.iter().max() {
            bitset.blocks.resize((max as usize / 64) + 1, 0);
        }
        for idx in indices {
            bitset.insert(idx);
        }
        Ok(bitset)
    }
}

use serde::{Deserialize, Serialize};

/// A highly-optimized, flat bitset for tracking boolean state across a large ID space (like map tiles).
/// It stores bits sequentially in a flat `Vec<u64>` to eliminate pointer chasing and cache misses.
///
/// For network efficiency via Serde, `serialize` and `deserialize` convert this dense
/// array into a sparse `Vec<u32>` payload, sending only the active indices over the wire.
#[derive(Clone, Debug, Default)]
pub struct DenseBitSet {
    /// Each u64 holds 64 bits. Total length = (max_capacity + 63) / 64.
    pub blocks: Vec<u64>,
    /// Directory of non-empty data blocks. One bit represents one `blocks` entry.
    /// This stays local and is not part of the sparse wire representation.
    non_empty_blocks: Vec<u64>,
}

impl PartialEq for DenseBitSet {
    fn eq(&self, other: &Self) -> bool {
        self.blocks == other.blocks
    }
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
        self.ensure_directory();

        let old = self.blocks[block];
        let mask = 1 << bit;
        self.blocks[block] = old | mask;
        if old == 0 {
            self.non_empty_blocks[block / 64] |= 1 << (block % 64);
        }
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
                let new = old & !mask;
                self.blocks[block] = new;
                if new == 0 && block / 64 < self.non_empty_blocks.len() {
                    self.non_empty_blocks[block / 64] &= !(1 << (block % 64));
                }
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
        self.non_empty_block_indices().flat_map(|b_idx| {
            let block = self.blocks[b_idx];
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
        self.sample_ones_with_work(start_idx, limit, out);
    }

    /// Same as [`Self::sample_ones`], returning the number of non-empty data
    /// blocks considered. The count is for local diagnostics only.
    pub fn sample_ones_with_work(&self, start_idx: u32, limit: usize, out: &mut Vec<u32>) -> u64 {
        out.clear();
        if limit == 0 || self.blocks.is_empty() {
            return 0;
        }

        let block_count = self.blocks.len();
        let start_block = (start_idx as usize / 64) % block_count;
        let start_bit = start_idx as usize % 64;
        let mut examined = 0;

        for block_idx in self.non_empty_block_indices() {
            if block_idx < start_block {
                continue;
            }
            examined += 1;
            let mut bits = self.blocks[block_idx];
            if block_idx == start_block && start_bit > 0 {
                bits &= u64::MAX << start_bit;
            }
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                out.push((block_idx * 64 + bit) as u32);
                if out.len() == limit {
                    return examined;
                }
                bits &= bits - 1;
            }
        }

        for block_idx in self.non_empty_block_indices() {
            if block_idx >= start_block {
                break;
            }
            examined += 1;
            let mut bits = self.blocks[block_idx];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                out.push((block_idx * 64 + bit) as u32);
                if out.len() == limit {
                    return examined;
                }
                bits &= bits - 1;
            }
        }

        if start_bit > 0 && out.len() < limit && self.blocks[start_block] != 0 {
            examined += 1;
            let mut bits = self.blocks[start_block] & ((1u64 << start_bit) - 1);
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                out.push((start_block * 64 + bit) as u32);
                if out.len() == limit {
                    return examined;
                }
                bits &= bits - 1;
            }
        }
        examined
    }

    /// Return the first set index at or after `start_idx`, wrapping once.
    pub fn first_one_from(&self, start_idx: u32) -> Option<u32> {
        self.first_one_from_with_work(start_idx).0
    }

    /// Same as [`Self::first_one_from`], returning the number of non-empty data
    /// blocks considered for local diagnostics.
    pub fn first_one_from_with_work(&self, start_idx: u32) -> (Option<u32>, u64) {
        if self.blocks.is_empty() {
            return (None, 0);
        }

        let block_count = self.blocks.len();
        let start_block = (start_idx as usize / 64) % block_count;
        let start_bit = start_idx as usize % 64;
        let mut examined = 0;
        for block_idx in self.non_empty_block_indices() {
            if block_idx < start_block {
                continue;
            }
            examined += 1;
            let mut bits = self.blocks[block_idx];
            if block_idx == start_block && start_bit > 0 {
                bits &= u64::MAX << start_bit;
            }
            if bits != 0 {
                return (
                    Some((block_idx * 64 + bits.trailing_zeros() as usize) as u32),
                    examined,
                );
            }
        }

        for block_idx in self.non_empty_block_indices() {
            if block_idx >= start_block {
                break;
            }
            examined += 1;
            let bits = self.blocks[block_idx];
            if bits != 0 {
                return (
                    Some((block_idx * 64 + bits.trailing_zeros() as usize) as u32),
                    examined,
                );
            }
        }

        if start_bit > 0 && self.blocks[start_block] != 0 {
            examined += 1;
            let bits = self.blocks[start_block] & ((1u64 << start_bit) - 1);
            if bits != 0 {
                return (
                    Some((start_block * 64 + bits.trailing_zeros() as usize) as u32),
                    examined,
                );
            }
        }
        (None, examined)
    }

    /// Counts total set bits.
    pub fn count_ones(&self) -> usize {
        self.non_empty_block_indices()
            .map(|idx| self.blocks[idx].count_ones() as usize)
            .sum()
    }

    /// Returns true if no bits are set.
    pub fn is_empty(&self) -> bool {
        self.non_empty_blocks.iter().all(|&b| b == 0)
    }

    /// Rebuilds the local directory after a caller has edited `blocks` directly.
    pub fn rebuild_index(&mut self) {
        self.non_empty_blocks
            .resize((self.blocks.len() + 63) / 64, 0);
        self.non_empty_blocks.fill(0);
        for (idx, &block) in self.blocks.iter().enumerate() {
            if block != 0 {
                self.non_empty_blocks[idx / 64] |= 1 << (idx % 64);
            }
        }
    }

    fn ensure_directory(&mut self) {
        let needed = (self.blocks.len() + 63) / 64;
        if self.non_empty_blocks.len() < needed {
            self.non_empty_blocks.resize(needed, 0);
        }
    }

    fn non_empty_block_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.non_empty_blocks
            .iter()
            .enumerate()
            .flat_map(|(directory_idx, &directory)| {
                let mut bits = directory;
                std::iter::from_fn(move || {
                    if bits == 0 {
                        None
                    } else {
                        let bit = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        Some(directory_idx * 64 + bit)
                    }
                })
            })
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

    #[test]
    fn sparse_high_block_sampling_keeps_order_after_remove() {
        let mut bits = DenseBitSet::new();
        bits.insert(1);
        bits.insert(50_000);
        bits.insert(100_000);

        let mut out = Vec::new();
        bits.sample_ones(50_000, 3, &mut out);
        assert_eq!(out, [50_000, 100_000, 1]);

        bits.remove(50_000);
        bits.sample_ones(50_000, 3, &mut out);
        assert_eq!(out, [100_000, 1]);
        assert_eq!(bits.first_one_from(50_000), Some(100_000));
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

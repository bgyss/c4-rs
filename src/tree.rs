use crate::id::Id;
use std::io::{self, Read};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree {
    data: Vec<u8>,
}

#[derive(Debug)]
pub enum TreeError {
    Io(io::Error),
    InvalidTree,
}

impl From<io::Error> for TreeError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl Tree {
    pub fn new(ids: &[Id]) -> Self {
        let mut size = 1usize;
        let mut len = ids.len();
        while len > 1 {
            size += len;
            len = (len + 1) / 2;
        }

        let mut data = vec![0; size * 64];
        let offset = data.len().saturating_sub(ids.len() * 64);
        for (i, id) in ids.iter().enumerate() {
            data[offset + i * 64..offset + (i + 1) * 64].copy_from_slice(&id.0);
        }
        Self { data }
    }

    pub fn bytes(&mut self) -> &[u8] {
        if !self.valid() {
            self.compute();
        }
        &self.data
    }

    pub fn id(&mut self) -> Id {
        if !self.valid() {
            return self.compute();
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(&self.data[..64]);
        Id(out)
    }

    pub fn len(&self) -> usize {
        list_size(self.data.len() / 64)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn valid(&self) -> bool {
        self.data.iter().take(64).any(|b| *b != 0)
    }

    fn compute(&mut self) -> Id {
        let row_ranges = build_row_ranges(self.data.len() / 64);
        for row in (0..row_ranges.len() - 1).rev() {
            let child = row_ranges[row + 1].clone();
            let parent = row_ranges[row].clone();
            let mut out_pos = parent.start;
            let mut pos = child.start;
            while pos < child.end {
                if pos + 128 > child.end {
                    let block: Vec<u8> = self.data[pos..pos + 64].to_vec();
                    self.data[out_pos..out_pos + 64].copy_from_slice(&block);
                } else {
                    let mut left = [0u8; 64];
                    let mut right = [0u8; 64];
                    left.copy_from_slice(&self.data[pos..pos + 64]);
                    right.copy_from_slice(&self.data[pos + 64..pos + 128]);
                    let summed = Id(left).sum(Id(right));
                    self.data[out_pos..out_pos + 64].copy_from_slice(&summed.0);
                }
                out_pos += 64;
                pos += 128;
            }
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(&self.data[..64]);
        Id(out)
    }
}

impl std::fmt::Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut copy = self.clone();
        copy.bytes();
        for chunk in copy.data.chunks_exact(64) {
            let mut id = [0u8; 64];
            id.copy_from_slice(chunk);
            write!(f, "{}", Id(id))?;
        }
        Ok(())
    }
}

pub fn read_tree<R: Read>(mut reader: R) -> Result<Tree, TreeError> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    if data.len() < 192 || data.len() % 64 != 0 {
        return Err(TreeError::InvalidTree);
    }
    let mut root = [0u8; 64];
    let mut left = [0u8; 64];
    let mut right = [0u8; 64];
    root.copy_from_slice(&data[..64]);
    left.copy_from_slice(&data[64..128]);
    right.copy_from_slice(&data[128..192]);
    if Id(left).sum(Id(right)) != Id(root) {
        return Err(TreeError::InvalidTree);
    }
    Ok(Tree { data })
}

fn build_row_ranges(total_ids: usize) -> Vec<std::ops::Range<usize>> {
    let mut width = list_size(total_ids);
    let mut start_ids = total_ids - width;
    let mut ranges = Vec::new();
    while width > 0 {
        ranges.push(start_ids * 64..(start_ids + width) * 64);
        if width == 1 {
            break;
        }
        width = (width + 1) / 2;
        start_ids -= width;
    }
    ranges.reverse();
    ranges
}

fn list_size(total: usize) -> usize {
    let max = (total + 1) / 2;
    let mut min = max.saturating_sub(usize::BITS as usize - total.leading_zeros() as usize);
    if tree_size(min) == total {
        return min;
    }
    if tree_size(max) == total {
        return max;
    }
    let mut hi = max;
    loop {
        let length = (min + hi) / 2;
        let size = tree_size(length);
        if size == total {
            return length;
        }
        if size > total {
            hi = length;
        } else {
            min = length;
        }
    }
}

fn tree_size(mut len: usize) -> usize {
    let mut total = 1;
    while len > 1 {
        total += len;
        len = (len + 1) / 2;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::identify_bytes;

    #[test]
    fn tree_round_trips() {
        let mut ids: Vec<Id> = (0u32..32)
            .map(|i| identify_bytes(&i.to_le_bytes()))
            .collect();
        ids.sort();
        ids.dedup();
        let mut tree = Tree::new(&ids);
        let root = tree.id();
        let data = tree.bytes().to_vec();
        let mut read = read_tree(&data[..]).unwrap();
        assert_eq!(read.id(), root);
        assert_eq!(read.len(), ids.len());
    }
}

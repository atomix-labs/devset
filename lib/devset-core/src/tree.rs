//! File sets held in one buffer.

use alloc::collections::BTreeMap;
use core::ops::Range;
use std::io;

use crate::digest::Digest;
use crate::path::RelPath;

/// Files keyed by path, their bytes stored back to back in one buffer.
#[derive(Debug, Default)]
pub(crate) struct Tree {
    /// Every file's bytes.
    buf: Vec<u8>,
    /// Each file's span of `buf`.
    index: BTreeMap<RelPath, Range<usize>>,
}

impl Tree {
    /// The bytes at `path`.
    #[must_use]
    pub(crate) fn get(&self, path: &RelPath) -> Option<&[u8]> {
        self.buf.get(self.index.get(path)?.clone())
    }

    /// Whether it holds `path`.
    pub(crate) fn contains(&self, path: &RelPath) -> bool {
        self.index.contains_key(path)
    }

    /// Every file, in path order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&RelPath, &[u8])> {
        self.index.iter().filter_map(|(path, span)| Some((path, self.buf.get(span.clone())?)))
    }

    /// Digest over every path and its content, independent of insertion order.
    #[must_use]
    pub(crate) fn digest(&self) -> Digest {
        let mut hasher = blake3::Hasher::new();
        for (path, bytes) in self.iter() {
            hasher.update(path.as_str().as_bytes());
            hasher.update(&[0]);
            hasher.update(Digest::of(bytes).as_bytes());
        }
        Digest::new(hasher.finalize())
    }

    /// Adds `path` with a copy of `bytes`.
    pub(crate) fn put(&mut self, path: RelPath, bytes: &[u8]) {
        let start = self.buf.len();
        self.buf.extend_from_slice(bytes);
        self.index.insert(path, start..self.buf.len());
    }

    /// Adds `path`, its bytes appended to the buffer by `fill`.
    pub(crate) fn insert<F>(&mut self, path: RelPath, fill: F) -> io::Result<()>
    where
        F: FnOnce(&mut Vec<u8>) -> io::Result<()>,
    {
        let start = self.buf.len();
        fill(&mut self.buf)?;
        self.index.insert(path, start..self.buf.len());
        Ok(())
    }
}

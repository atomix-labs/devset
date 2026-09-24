//! File sets held in one buffer.

use core::ops::Range;
use std::io;

use crate::digest::Digest;
use crate::path::RelPath;

/// Files keyed by path, their bytes stored back to back in one buffer.
#[derive(Debug, Default)]
pub(crate) struct Tree {
    /// Every file's bytes.
    buf: Vec<u8>,
    /// Each file's span of `buf`, sorted by path.
    index: Vec<(RelPath, Range<usize>)>,
}

impl Tree {
    /// The bytes at `path`.
    #[must_use]
    pub(crate) fn get(&self, path: &RelPath) -> Option<&[u8]> {
        let at = self.index.binary_search_by(|(p, _)| p.cmp(path)).ok()?;
        let (_, span) = self.index.get(at)?;
        self.buf.get(span.clone())
    }

    /// Every file, in path order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&RelPath, &[u8])> {
        self.index
            .iter()
            .filter_map(|(path, span)| Some((path, self.buf.get(span.clone())?)))
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
        self.place(path, start);
    }

    /// Adds `path`, its bytes appended to the buffer by `fill`.
    pub(crate) fn insert<F>(&mut self, path: RelPath, fill: F) -> io::Result<()>
    where
        F: FnOnce(&mut Vec<u8>) -> io::Result<()>,
    {
        let start = self.buf.len();
        fill(&mut self.buf)?;
        self.place(path, start);
        Ok(())
    }

    /// Indexes `path` as the bytes from `start` to the end of the buffer, replacing its entry.
    fn place(&mut self, path: RelPath, start: usize) {
        let span = start..self.buf.len();
        match self.index.binary_search_by(|(p, _)| p.cmp(&path)) {
            Ok(at) => {
                if let Some(entry) = self.index.get_mut(at) {
                    entry.1 = span;
                }
            }
            Err(at) => self.index.insert(at, (path, span)),
        }
    }
}

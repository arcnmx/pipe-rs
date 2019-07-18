#![deny(missing_docs)]
#![doc(html_root_url = "http://docs.rs/pipe/0.2.0")]
#![cfg_attr(feature = "unstable-doc-cfg", feature(doc_cfg))]

//! Synchronous in-memory pipe
//!
//! ## Example
//!
//! ```
//! use std::thread::spawn;
//! use std::io::{Read, Write};
//!
//! let (mut read, mut write) = pipe::pipe();
//!
//! let message = "Hello, world!";
//! spawn(move || write.write_all(message.as_bytes()).unwrap());
//!
//! let mut s = String::new();
//! read.read_to_string(&mut s).unwrap();
//!
//! assert_eq!(&s, message);
//! ```

#[cfg(feature="readwrite")]
extern crate readwrite;
extern crate crossbeam_channel;

use crossbeam_channel::{Sender, Receiver};
use std::io::{self, BufRead, Read, Write};
use std::cmp::min;

/// The `Read` end of a pipe (see `pipe()`)
pub struct PipeReader(Receiver<Vec<u8>>, Vec<u8>);

/// The `Write` end of a pipe (see `pipe()`)
#[derive(Clone)]
pub struct PipeWriter(Sender<Vec<u8>>);

/// Creates a synchronous memory pipe
pub fn pipe() -> (PipeReader, PipeWriter) {
    let (tx, rx) = crossbeam_channel::bounded(0);

    (PipeReader(rx, Vec::new()), PipeWriter(tx))
}

/// Creates a pair of pipes for bidirectional communication, a bit like UNIX's `socketpair(2)`.
#[cfg(feature = "bidirectional")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "bidirectional")))]
pub fn bipipe() -> (readwrite::ReadWrite<PipeReader, PipeWriter>, readwrite::ReadWrite<PipeReader, PipeWriter>) {
    let (r1,w1) = pipe();
    let (r2,w2) = pipe();
    ((r1,w2).into(), (r2,w1).into())
}

impl PipeWriter {
    /// Extracts the inner `SyncSender` from the writer
    pub fn into_inner(self) -> Sender<Vec<u8>> {
        self.0
    }
}

impl PipeReader {
    /// Extracts the inner `Receiver` from the writer, and any pending buffered data
    pub fn into_inner(self) -> (Receiver<Vec<u8>>, Vec<u8>) {
        (self.0, self.1)
    }
}

impl BufRead for PipeReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        while self.1.is_empty() {
            match self.0.recv() {
                // The only existing error is EOF
                Err(_) => break,
                Ok(data) => self.1 = data,
            }
        }

        return Ok(&self.1);
    }

    fn consume(&mut self, amt: usize) {
        self.1.drain(..amt);
    }
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let internal = self.fill_buf()?;

        let len = min(buf.len(), internal.len());
        if len > 0 {
            buf[..len].copy_from_slice(&internal[..len]);
            self.consume(len);
        }
        Ok(len)
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let data = buf.to_vec();

        self.0.send(data)
            .map(|_| buf.len())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "pipe reader has been dropped"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread::spawn;
    use std::io::{Read, Write};
    use super::*;

    #[test]
    fn pipe_reader() {
        let i = b"hello there";
        let mut o = Vec::with_capacity(i.len());
        let (mut r, mut w) = pipe();
        let guard = spawn(move || {
            w.write_all(&i[..5]).unwrap();
            w.write_all(&i[5..]).unwrap();
            drop(w);
        });

        r.read_to_end(&mut o).unwrap();
        assert_eq!(i, &o[..]);

        guard.join().unwrap();
    }

    #[test]
    fn pipe_writer_fail() {
        let i = b"hi";
        let (r, mut w) = pipe();
        let guard = spawn(move || {
            drop(r);
        });

        assert!(w.write_all(i).is_err());

        guard.join().unwrap();
    }

    #[test]
    fn small_reads() {
        let block_cnt = 20;
        const BLOCK: usize = 20;
        let (mut r, mut w) = pipe();
        let guard = spawn(move || {
            for _ in 0..block_cnt {
                let data = &[0; BLOCK];
                w.write_all(data).unwrap();
            }
        });

        let mut buff = [0; BLOCK / 2];
        let mut read = 0;
        while let Ok(size) = r.read(&mut buff) {
            // 0 means EOF
            if size == 0 {
                break;
            }
            read += size;
        }
        assert_eq!(block_cnt * BLOCK, read);

        guard.join().unwrap();
    }
}

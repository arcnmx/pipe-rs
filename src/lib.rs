#![deny(missing_docs)]

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

extern crate resize_slice;

use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use std::io::{self, Read, Write};
use std::cmp::min;
use std::ptr::copy_nonoverlapping;
use resize_slice::ResizeSlice;

/// The `Read` end of a pipe (see `pipe()`)
pub struct PipeReader(Receiver<Vec<u8>>, Vec<u8>);

/// The `Write` end of a pipe (see `pipe()`)
#[derive(Clone)]
pub struct PipeWriter(SyncSender<Vec<u8>>);

/// Creates a synchronous memory pipe
pub fn pipe() -> (PipeReader, PipeWriter) {
    let (tx, rx) = sync_channel(0);

    (PipeReader(rx, Vec::new()), PipeWriter(tx))
}

impl PipeWriter {
    /// Extracts the inner `SyncSender` from the writer
    pub fn into_inner(self) -> SyncSender<Vec<u8>> {
        self.0
    }
}

impl PipeReader {
    /// Extracts the inner `Receiver` from the writer, and any pending buffered data
    pub fn into_inner(self) -> (Receiver<Vec<u8>>, Vec<u8>) {
        (self.0, self.1)
    }
}

impl Read for PipeReader {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let buf_len = min(buf.len(), self.1.len());
        unsafe { copy_nonoverlapping(self.1.as_ptr(), buf.as_mut_ptr(), buf_len); }
        buf.resize_from(buf_len);

        if buf.len() == 0 {
            return Ok(buf_len)
        }

        match self.0.recv() {
            Err(_) => Ok(0),
            Ok(data) => {
                let len = min(buf.len(), data.len());
                unsafe { copy_nonoverlapping(data.as_ptr(), buf.as_mut_ptr(), len); }
                self.1.reserve(data.len() - len);
                unsafe { copy_nonoverlapping(data[len..].as_ptr(), self.1.as_mut_ptr(), data.len() - len); }
                Ok(buf_len + len)
            },
        }
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut data = Vec::with_capacity(buf.len());
        unsafe {
            copy_nonoverlapping(buf.as_ptr(), data.as_mut_ptr(), buf.len());
            data.set_len(buf.len());
        }

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
}

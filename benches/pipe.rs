#[macro_use]
extern crate criterion;
extern crate os_pipe;
extern crate pipe;

use criterion::{black_box, Bencher, Criterion, ParameterizedBenchmark, Throughput};
use std::convert::TryInto;
use std::io::prelude::*;
use std::io::BufWriter;
use std::thread;

const TOTAL_TO_SEND: usize = 1 * 1024 * 1024;

fn send_recv_size<F, R, W>(mut f: F) -> impl FnMut(&mut Bencher, &&(usize, usize))
where
    F: FnMut() -> (R, W),
    F: Send + 'static,
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    return move |b: &mut Bencher, &&(size, reads)| {
        let f = &mut f;
        let buf: Vec<u8> = (0..size).map(|i| i as u8).collect();
        b.iter(move || {
            let (mut reader, mut writer) = f();
            let t = thread::spawn(move || {
                let mut buf = vec![0; size / reads];
                while let Ok(_) = reader.read_exact(&mut buf) {}
            });

            for _ in 0..(TOTAL_TO_SEND / size) {
                writer.write_all(black_box(&buf[..])).unwrap();
            }
            writer.flush().unwrap();
            drop(writer);
            t.join().expect("writing failed");
        })
    };
}

fn pipe_send(c: &mut Criterion) {
    const KB: usize = 1024;
    const SIZES: &[(usize, usize)] = &[
        (4 * KB, 1),
        (4 * KB, 16),
        (8 * KB, 1),
        (16 * KB, 1),
        (32 * KB, 1),
        (64 * KB, 1),
        (64 * KB, 16),
    ];
    let bench = ParameterizedBenchmark::new(
        "pipe-rs",
        send_recv_size(|| pipe::pipe()),
        SIZES,
    );
    let bench = bench
        .with_function("pipe-rs-buffered", send_recv_size(|| pipe::pipe_buffered()))
        .with_function("pipe-rs-bufwrite", send_recv_size(|| {
            let (r, w) = pipe::pipe();
            (r, BufWriter::new(w))
        }))
        .with_function("os_pipe", send_recv_size(|| os_pipe::pipe().unwrap()))
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);
}

criterion_group!(benches, pipe_send);
criterion_main!(benches);

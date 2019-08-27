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

fn send_recv_size<F, R, W>(mut f: F, reads: usize) -> impl FnMut(&mut Bencher, &&usize)
where
    F: FnMut() -> (R, W),
    F: Send + 'static,
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    return move |b: &mut Bencher, &&size: &&usize| {
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
    const SIZES: &[usize] = &[4 * KB, 8 * KB, 16 * KB, 32 * KB, 64 * KB];

    let bench = ParameterizedBenchmark::new( "pipe-rs", send_recv_size(|| pipe::pipe(), 1), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "pipe-rs (16 reads per write)", send_recv_size(|| pipe::pipe(), 16), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "pipe-rs buffered", send_recv_size(|| pipe::pipe_buffered(), 1), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "pipe-rs buffered (16 reads per write)", send_recv_size(|| pipe::pipe_buffered(), 16), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "pipe-rs BufWriter", send_recv_size(|| { let (r, w) = pipe::pipe(); (r, BufWriter::new(w))}, 1), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "pipe-rs BufWriter (16 reads per write)", send_recv_size(|| { let (r, w) = pipe::pipe(); (r, BufWriter::new(w))}, 16), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "os_pipe", send_recv_size(|| os_pipe::pipe().unwrap(), 1), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);

    let bench = ParameterizedBenchmark::new( "os_pipe (16 reads per write)", send_recv_size(|| os_pipe::pipe().unwrap(), 16), SIZES)
        .throughput(|_| Throughput::Bytes(TOTAL_TO_SEND.try_into().unwrap()));
    c.bench("pipe_send", bench);
}

criterion_group!(benches, pipe_send);
criterion_main!(benches);

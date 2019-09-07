//! Benchmark for downloading files over localhost.

use criterion::*;
use isahc_benchmarks::TestServer;
use std::io::{sink, Write};

static DATA: [u8; 0x10000] = [1; 0x10000]; // 64K

fn benchmark(c: &mut Criterion) {
    c.bench_function("download 64K: curl", move |b| {
        let server = TestServer::static_response(&DATA);
        let endpoint = server.endpoint();

        b.iter_batched(
            || {
                let mut easy = curl::easy::Easy::new();
                easy.url(&endpoint).unwrap();
                easy
            },
            |mut easy| {
                let mut sink = sink();
                let mut transfer = easy.transfer();

                transfer
                    .write_function(|bytes| {
                        sink.write_all(bytes).unwrap();
                        Ok(bytes.len())
                    })
                    .unwrap();

                transfer.perform().unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("download 64K: isahc", move |b| {
        use isahc::prelude::*;

        let server = TestServer::static_response(&DATA);
        let endpoint = server.endpoint();

        b.iter_batched(
            || isahc::HttpClient::new().unwrap(),
            |client| {
                client.get(&endpoint).unwrap().copy_to(sink()).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("download 64K: reqwest", move |b| {
        let server = TestServer::static_response(&DATA);
        let endpoint = server.endpoint();

        b.iter_batched(
            || reqwest::Client::new(),
            |client| {
                client
                    .get(&endpoint)
                    .send()
                    .unwrap()
                    .copy_to(&mut sink())
                    .unwrap();
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

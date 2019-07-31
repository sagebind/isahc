//! Benchmark for downloading files over localhost.

use criterion::*;
use utilities::server;

static DATA: [u8; 0x10000] = [1; 0x10000]; // 64K

fn benchmark(c: &mut Criterion) {
    c.bench_function("download 64K: curl", move |b| {
        let server = server::spawn(|_| server::static_response(&DATA));
        let endpoint = server.endpoint();

        b.iter_batched(
            || {
                let mut easy = curl::easy::Easy::new();
                easy.url(&endpoint).unwrap();
                easy
            },
            |mut easy| {
                let mut body = Vec::new();
                let mut transfer = easy.transfer();

                transfer
                    .write_function(|bytes| {
                        body.extend_from_slice(bytes);
                        Ok(bytes.len())
                    })
                    .unwrap();

                transfer.perform().unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("download 64K: isahc", move |b| {
        use std::io::Read;

        let server = server::spawn(|_| server::static_response(&DATA));
        let endpoint = server.endpoint();

        b.iter_batched(
            || isahc::Client::new(),
            |client| {
                let mut body = Vec::new();

                let mut response = client.get(&endpoint).unwrap();
                response.body_mut().read_to_end(&mut body).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("download 64K: reqwest", move |b| {
        let server = server::spawn(|_| server::static_response(&DATA));
        let endpoint = server.endpoint();

        b.iter_batched(
            || reqwest::Client::new(),
            |client| {
                let mut body = Vec::new();

                client
                    .get(&endpoint)
                    .send()
                    .unwrap()
                    .copy_to(&mut body)
                    .unwrap();
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

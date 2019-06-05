//! Benchmark for downloading files over localhost.

#![feature(async_await)]

use criterion::*;

fn benchmark(c: &mut Criterion) {
    let server = utilities::server::spawn(|request| {
        static DATA: [u8; 0x10000] = [1; 0x10000]; // 64K

        utilities::rouille::Response::from_data("application/octet-stream", DATA.to_vec())
    });

    {
        let endpoint = server.endpoint();

        c.bench_function("download 64K: chttp", move |b| {
            use std::io::Read;

            b.iter(|| {
                let mut body = Vec::new();

                let mut response = chttp::get(&endpoint).unwrap();
                response.body_mut().read_to_end(&mut body).unwrap();
            })
        });
    }

    {
        let endpoint = server.endpoint();

        c.bench_function("download 64K: curl", move |b| {
            b.iter_batched(
                || {
                    let mut easy = curl::easy::Easy::new();
                    easy.url(&endpoint).unwrap();
                    easy
                },
                |mut easy| {
                    let mut body = Vec::new();
                    let mut transfer = easy.transfer();

                    transfer.write_function(|bytes| {
                        body.extend_from_slice(bytes);
                        Ok(bytes.len())
                    }).unwrap();

                    transfer.perform().unwrap();
                },
                BatchSize::SmallInput,
            )
        });
    }
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

//! Benchmark for downloading files over localhost.

use httpmock::prelude::*;
use isahc::prelude::*;
use std::io::{Write, sink};

static DATA: [u8; 0x10000] = [1; 0x10000]; // 64K

fn main() {
    divan::main();
}

#[divan::bench(sample_count = 1000)]
fn download_64k_curl(bencher: divan::Bencher) {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET");
        then.status(200).body(&DATA);
    });
    let endpoint = server.base_url();

    bencher
        .with_inputs(|| {
            let mut easy = curl::easy::Easy::new();
            easy.url(&endpoint).unwrap();
            easy
        })
        .bench_values(|mut easy| {
            let mut sink = sink();
            let mut transfer = easy.transfer();

            transfer
                .write_function(|bytes| {
                    sink.write_all(bytes).unwrap();
                    Ok(bytes.len())
                })
                .unwrap();

            transfer.perform().unwrap();
        });

    drop(mock);
}

#[divan::bench(sample_count = 1000)]
fn download_64k_isahc(bencher: divan::Bencher) {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET");
        then.status(200).body(&DATA);
    });
    let endpoint = server.base_url();

    bencher
        .with_inputs(|| isahc::HttpClient::new().unwrap())
        .bench_values(|client| {
            client.get(&endpoint).unwrap().copy_to(sink()).unwrap();
        });

    drop(mock);
}

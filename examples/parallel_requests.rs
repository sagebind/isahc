//! In this example, we demonstrate Isahc's ability to run many requests
//! simultaneously with no extra cost. Concurrent requests may be made in the
//! same thread, or from different threads as in this example.
//!
//! We're using Rayon here to make parallelism easy.
use isahc::prelude::*;
use rayon::prelude::*;
use std::env;
use std::time::Instant;

fn main() -> Result<(), isahc::Error> {
    let count = env::args()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(100);

    let urls: Vec<String> = (0..count)
        .map(|i| format!("https://httpbin.org/anything/{:03}", i))
        .collect();
    let client = HttpClient::new();

    let start = Instant::now();

    // Iterate over each URL and send a request in parallel.
    urls.par_iter()
        .try_for_each(|url| {
            let start = Instant::now();
            let response = client.get(url)?;
            let end = Instant::now();
            println!(
                "{}: {} in {:?}",
                &url,
                response.status(),
                end.duration_since(start)
            );

            Ok(())
        })
        .map(|_| {
            let end = Instant::now();
            println!("Ran {} requests in {:?}", count, end.duration_since(start));
        })
}

//! This example highlights Isahc's async streaming capabilities by implementing
//! a program that aborts downloading a response if it contains the byte `0x3F`
//! (ASCII "?").

use futures::{executor::block_on, io::AsyncReadExt};

fn main() -> Result<(), isahc::Error> {
    block_on(async {
        // Open a response stream.
        let response = isahc::get_async("https://www.rust-lang.org").await?;

        let mut buf = [0; 8192];
        let mut offset = 0;
        let mut reader = response.into_body();

        // Set up a loop where we continuously read from the stream.
        loop {
            match reader.read(&mut buf).await? {
                // Zero bytes read, we hit EOF with no question marks.
                0 => {
                    println!("Download complete! No '?' byte of all {} bytes.", offset);
                    return Ok(());
                }
                // At least one byte was read.
                len => {
                    // Check to dee if there's any question marks this time
                    // around.
                    for &byte in &buf[..len] {
                        if byte == b'?' {
                            println!("Abort, saw a '?' at offset {}!", offset);
                            return Ok(());
                        }
                        // Keep track of how many bytes we've checked so far.
                        offset += 1;
                    }
                }
            }
        }

        // If we did not read the entire stream before returning, when the
        // response is dropped the download will be aborted.
    })
}

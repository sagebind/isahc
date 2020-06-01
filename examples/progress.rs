//! A simple command line utility that downloads a file into the void. It
//! demonstrates how the metrics API can be used to implement an interactive
//! progress bar.
//!
//! Command line options are parsed with [structopt] and the progress bar itself
//! rendered with [indicatif], both excellent libraries for writing command line
//! programs!
//!
//! [indicatif]: https://github.com/mitsuhiko/indicatif
//! [structopt]: https://github.com/TeXitoi/structopt

use indicatif::{FormattedDuration, HumanBytes, ProgressBar, ProgressStyle};
use isahc::prelude::*;
use std::io::Read;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    url: http::Uri,
}

fn main() -> Result<(), isahc::Error> {
    let options = Options::from_args();

    let bar = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {bytes:>7}/{total_bytes:7} {msg}"),
    );

    let mut response = Request::get(options.url).metrics(true).body(())?.send()?;
    let metrics = response.metrics().unwrap().clone();
    let body = response.body_mut();
    let mut buf = [0; 16384 * 4];

    loop {
        match body.read(&mut buf) {
            Ok(0) => {
                bar.finish();
                break;
            }
            Ok(_) => {
                bar.set_position(metrics.download_progress().0);
                bar.set_length(metrics.download_progress().1);
                bar.set_message(&format!(
                    "time: {}  speed: {}/sec",
                    FormattedDuration(metrics.total_time()),
                    HumanBytes(metrics.download_speed() as u64),
                ));
            }
            Err(e) => {
                bar.finish_at_current_pos();
                eprintln!("Error: {}", e);
                return Ok(());
            }
        }
    }

    Ok(())
}

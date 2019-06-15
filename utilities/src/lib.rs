use std::env;
use std::sync::Once;

pub use rouille;

pub mod server;

pub fn logging() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        env::set_var("RUST_BACKTRACE", "1");
        env::set_var("RUST_LOG", "chttp=trace,curl=debug");
        env_logger::init();
    });
}

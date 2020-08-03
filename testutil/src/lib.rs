mod mock;

pub use mock::*;

#[ctor::ctor]
fn init_impl() {
    tracing_subscriber::fmt::init();
}

pub fn init() {
    init_impl();
}

#[macro_export]
macro_rules! init {
    () => {
        #[no_mangle]
        pub extern "C" fn init_local() {
            $crate::init();
        }
    };
}

pub fn mock() -> Mock {
    Mock::new()
}

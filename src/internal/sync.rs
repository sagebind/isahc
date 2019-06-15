use std::sync::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct DropFlag {
    flag: AtomicBool,
    mutex: Mutex<()>,
    cvar: Condvar,
}

impl DropFlag {
    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    pub fn wait(&self) {
        let mut lock = self.mutex.lock().unwrap();
        while !self.is_set() {
            lock = self.cvar.wait(lock).unwrap();
        }
    }
}

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::SeqCst);
        self.cvar.notify_all();
    }
}

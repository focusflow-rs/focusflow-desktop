use std::sync::{Mutex, MutexGuard, OnceLock};

static IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn io_lock() -> MutexGuard<'static, ()> {
    IO_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn io_lock_works_in_normal_path() {
        let _guard = io_lock();
    }

    #[test]
    fn io_lock_recovers_from_poisoned_mutex() {
        let lock = IO_LOCK.get_or_init(|| Mutex::new(()));
        let _ = thread::spawn(move || {
            let _guard = lock.lock().expect("lock should work before poisoning");
            panic!("intentional poison");
        })
        .join();

        let _guard = io_lock();
    }
}

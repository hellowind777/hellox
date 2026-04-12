use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(test)]
pub(super) fn env_lock() -> MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("startup env test lock poisoned")
}

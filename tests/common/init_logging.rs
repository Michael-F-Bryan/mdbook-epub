use std::sync::Once;

static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();
    });
}

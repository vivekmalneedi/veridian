use std::sync::Once;

static INIT_LOGGER: Once = Once::new();

pub fn test_init() {
    INIT_LOGGER.call_once(|| {
        let _ = flexi_logger::Logger::with(flexi_logger::LogSpecification::info()).start();
    });
}

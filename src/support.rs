pub fn test_init() {
    let _ = flexi_logger::Logger::try_with_str("info").expect("init logger").start();
}

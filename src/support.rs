pub fn test_init() {
    // NOTE: I'll just do a unwrap here as the given string is valid.
    let _ = flexi_logger::Logger::try_with_str("info").unwrap().start();
}

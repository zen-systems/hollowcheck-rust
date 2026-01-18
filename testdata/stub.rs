// Test fixtures for hollowcheck - stub patterns.

/// Placeholder configuration type.
pub struct StubConfig {
    pub name: String,
}

/// A stub constant.
pub const DEFAULT_TIMEOUT: u32 = 30;

/// Stub function that does nothing useful.
/// TODO: implement actual processing logic
pub fn process_data(input: &str) -> Result<(), &'static str> {
    // FIXME: this is a placeholder implementation
    Ok(())
}

/// Another stub.
/// HACK: bypassing validation for now
pub fn validate_input(data: &[u8]) -> bool {
    // XXX: need to add proper validation
    true
}

/// Stub handler.
pub fn handle_request() -> Result<(), &'static str> {
    panic!("not implemented")
}

/// Demonstrates a stub pattern.
pub fn not_implemented_func() {
    // This function intentionally left empty
    unimplemented!()
}

/// Another unimplemented function.
pub fn another_stub() {
    todo!("add implementation")
}

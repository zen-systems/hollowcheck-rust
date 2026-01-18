// Test fixtures for hollowcheck - stub patterns.

#include <string>
#include <stdexcept>
#include <cstdint>

namespace testdata {

/**
 * Placeholder configuration type.
 */
struct StubConfig {
    std::string name;
};

// A stub constant.
constexpr int DEFAULT_TIMEOUT = 30;

/**
 * Stub function that does nothing useful.
 * TODO: implement actual processing logic
 */
int processData(const std::string& input) {
    // FIXME: this is a placeholder implementation
    return 0;
}

/**
 * Another stub.
 * HACK: bypassing validation for now
 */
bool validateInput(const uint8_t* data, size_t len) {
    // XXX: need to add proper validation
    return true;
}

/**
 * Stub handler.
 */
void handleRequest() {
    throw std::runtime_error("not implemented");
}

/**
 * Demonstrates a stub pattern.
 */
void notImplementedFunc() {
    // This function intentionally left empty
    return;
}

} // namespace testdata

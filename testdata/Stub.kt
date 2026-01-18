// Test fixtures for hollowcheck - stub patterns.
package testdata

/**
 * Placeholder configuration type.
 */
data class StubConfig(val name: String = "")

const val DEFAULT_TIMEOUT = 30

/**
 * Stub function that does nothing useful.
 * TODO: implement actual processing logic
 */
fun processData(input: String): Unit {
    // FIXME: this is a placeholder implementation
    return
}

/**
 * Another stub.
 * HACK: bypassing validation for now
 */
fun validateInput(data: ByteArray): Boolean {
    // XXX: need to add proper validation
    return true
}

/**
 * Stub handler.
 */
fun handleRequest() {
    throw NotImplementedError("not implemented")
}

/**
 * Demonstrates a stub pattern.
 */
fun notImplementedFunc() {
    // This function intentionally left empty
    TODO("add implementation")
}

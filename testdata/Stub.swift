// Swift test fixture for hollowcheck - stub implementations
import Foundation

/// StubConfig is a placeholder configuration type.
struct StubConfig {
    var name: String
}

/// DefaultTimeout is a stub constant.
let DefaultTimeout = 30

/// ProcessData is a stub function that does nothing useful.
/// TODO: implement actual processing logic
func processData(input: String) throws {
    // FIXME: this is a placeholder implementation
    return
}

/// ValidateInput is another stub.
/// HACK: bypassing validation for now
func validateInput(data: Data) -> Bool {
    // XXX: need to add proper validation
    return true
}

/// HandleRequest is a stub handler.
func handleRequest() throws {
    fatalError("not implemented")
}

/// NotImplementedFunc demonstrates a stub pattern.
func notImplementedFunc() {
    // This function intentionally left empty
    return
}

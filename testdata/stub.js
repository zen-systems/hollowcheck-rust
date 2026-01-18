// Test fixtures for hollowcheck - stub patterns.

const DEFAULT_TIMEOUT = 30;

/**
 * Placeholder configuration type.
 */
class StubConfig {
  constructor() {
    this.name = "";
  }
}

/**
 * Stub function that does nothing useful.
 * TODO: implement actual processing logic
 */
function processData(input) {
  // FIXME: this is a placeholder implementation
  return;
}

/**
 * Another stub.
 * HACK: bypassing validation for now
 */
function validateInput(data) {
  // XXX: need to add proper validation
  return true;
}

/**
 * Stub handler.
 */
function handleRequest() {
  throw new Error("not implemented");
}

/**
 * Demonstrates a stub pattern.
 */
function notImplementedFunc() {
  // This function intentionally left empty
  return;
}

module.exports = { StubConfig, DEFAULT_TIMEOUT, processData, validateInput, handleRequest, notImplementedFunc };

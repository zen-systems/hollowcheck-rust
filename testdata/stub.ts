// Test fixtures for hollowcheck - stub patterns.

interface StubConfig {
  name: string;
}

const DEFAULT_TIMEOUT = 30;

/**
 * Stub function that does nothing useful.
 * TODO: implement actual processing logic
 */
function processData(input: string): void {
  // FIXME: this is a placeholder implementation
  return;
}

/**
 * Another stub.
 * HACK: bypassing validation for now
 */
function validateInput(data: Uint8Array): boolean {
  // XXX: need to add proper validation
  return true;
}

/**
 * Stub handler.
 */
function handleRequest(): void {
  throw new Error("not implemented");
}

/**
 * Demonstrates a stub pattern.
 */
function notImplementedFunc(): void {
  // This function intentionally left empty
  return;
}

export { StubConfig, DEFAULT_TIMEOUT, processData, validateInput, handleRequest, notImplementedFunc };

// Test fixtures for hollowcheck - stub patterns.
package testdata;

/**
 * Placeholder configuration type.
 */
class StubConfig {
    String name;
}

public class Stub {
    public static final int DEFAULT_TIMEOUT = 30;

    /**
     * Stub function that does nothing useful.
     * TODO: implement actual processing logic
     */
    public static void processData(String input) {
        // FIXME: this is a placeholder implementation
        return;
    }

    /**
     * Another stub.
     * HACK: bypassing validation for now
     */
    public static boolean validateInput(byte[] data) {
        // XXX: need to add proper validation
        return true;
    }

    /**
     * Stub handler.
     */
    public static void handleRequest() {
        throw new UnsupportedOperationException("not implemented");
    }

    /**
     * Demonstrates a stub pattern.
     */
    public static void notImplementedFunc() {
        // This function intentionally left empty
        return;
    }
}

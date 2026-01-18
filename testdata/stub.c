// Test fixtures for hollowcheck - stub patterns.

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>

// Placeholder configuration type.
typedef struct {
    char* name;
} StubConfig;

// A stub constant.
#define DEFAULT_TIMEOUT 30

/**
 * Stub function that does nothing useful.
 * TODO: implement actual processing logic
 */
int process_data(const char* input) {
    // FIXME: this is a placeholder implementation
    return 0;
}

/**
 * Another stub.
 * HACK: bypassing validation for now
 */
bool validate_input(const unsigned char* data, size_t len) {
    // XXX: need to add proper validation
    return true;
}

/**
 * Stub handler.
 */
int handle_request(void) {
    fprintf(stderr, "not implemented\n");
    abort();
    return -1;
}

/**
 * Demonstrates a stub pattern.
 */
void not_implemented_func(void) {
    // This function intentionally left empty
    return;
}

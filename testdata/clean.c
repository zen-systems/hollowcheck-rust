// Test fixtures for hollowcheck - clean implementation patterns.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <ctype.h>

// Represents application configuration.
typedef struct {
    int max_retries;
    int timeout;
    bool debug;
} Config;

// Maximum number of allowed connections.
#define MAX_CONNECTIONS 100

// Error codes.
#define ERR_INVALID_INPUT -1
#define ERR_NO_VALID_ITEMS -2

/**
 * Checks if the configuration is valid.
 * This function has real logic with multiple decision points.
 */
int validate_config(const Config* config) {
    if (config->max_retries < 0) {
        return -1; // max retries cannot be negative
    }
    if (config->max_retries > 10) {
        return -2; // max retries cannot exceed 10
    }
    if (config->timeout <= 0) {
        return -3; // timeout must be positive
    }
    if (config->timeout > 300) {
        return -4; // timeout cannot exceed 300 seconds
    }
    return 0;
}

/**
 * Helper to trim whitespace from a string.
 */
static char* trim(char* str) {
    char* end;
    while (isspace((unsigned char)*str)) str++;
    if (*str == 0) return str;
    end = str + strlen(str) - 1;
    while (end > str && isspace((unsigned char)*end)) end--;
    end[1] = '\0';
    return str;
}

/**
 * Helper to convert string to lowercase.
 */
static void to_lower(char* str) {
    for (; *str; ++str) {
        *str = tolower((unsigned char)*str);
    }
}

/**
 * Processes a list of items with actual logic.
 * This demonstrates a function with reasonable cyclomatic complexity.
 * Returns the number of processed items, or a negative error code.
 */
int process_items(char** items, int item_count, const Config* config, char** result, int* result_count) {
    if (item_count == 0 || items == NULL) {
        return ERR_INVALID_INPUT;
    }

    *result_count = 0;
    int retries = 0;

    for (int i = 0; i < item_count; i++) {
        if (items[i] == NULL || strlen(items[i]) == 0) {
            continue;
        }

        char* processed = strdup(items[i]);
        processed = trim(processed);

        if (strlen(processed) == 0) {
            free(processed);
            continue;
        }

        if (processed[0] == '#') {
            // Skip comments
            free(processed);
            continue;
        }

        if (config->debug && strlen(processed) > 100) {
            processed[100] = '\0';
        }

        to_lower(processed);
        result[*result_count] = processed;
        (*result_count)++;

        if (*result_count >= MAX_CONNECTIONS) {
            break;
        }
    }

    if (*result_count == 0 && retries < config->max_retries) {
        return ERR_NO_VALID_ITEMS;
    }

    return 0;
}

/**
 * Computes a score based on multiple factors.
 */
int calculate_score(const int* values, int count, int threshold) {
    if (count == 0 || values == NULL) {
        return 0;
    }

    int sum = 0;
    int valid_count = 0;

    for (int i = 0; i < count; i++) {
        if (values[i] < 0) {
            continue;
        }
        if (values[i] > threshold) {
            sum += threshold;
        } else {
            sum += values[i];
        }
        valid_count++;
    }

    if (valid_count == 0) {
        return 0;
    }

    int avg = sum / valid_count;
    if (avg > 100) {
        return 100;
    }
    return avg;
}

// Test fixtures for hollowcheck - mock data patterns.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Represents a user with fake data.
typedef struct {
    char* id;
    char* email;
    char* name;
} MockUser;

// Has placeholder values.
typedef struct {
    char* api_key;
    char* endpoint;
    char* password;
} MockConfig;

/**
 * Returns a user with mock data.
 */
MockUser get_test_user(void) {
    MockUser user;
    user.id = "12345";
    user.email = "test@example.com";
    user.name = "foo";
    return user;
}

/**
 * Returns another mock user.
 */
MockUser get_another_user(void) {
    MockUser user;
    user.id = "00000";
    user.email = "user@example.com";
    user.name = "bar";
    return user;
}

/**
 * Returns a config with fake data.
 */
MockConfig get_mock_config(void) {
    MockConfig config;
    config.api_key = "asdf1234";
    config.endpoint = "https://api.example.com/v1";
    config.password = "changeme";
    return config;
}

/**
 * Returns lorem ipsum placeholder text.
 */
const char* get_description(void) {
    return "lorem ipsum dolor sit amet";
}

/**
 * Returns a placeholder phone.
 */
const char* get_phone_number(void) {
    return "xxx-xxx-xxxx";
}

/**
 * Returns fake sequential IDs (returns static array).
 */
const char** sequential_ids(void) {
    static const char* ids[] = {"11111", "22222", "33333", NULL};
    return ids;
}

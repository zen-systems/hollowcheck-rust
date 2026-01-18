// Test fixtures for hollowcheck - mock data patterns.

#include <string>
#include <vector>

namespace testdata {

/**
 * Represents a user with fake data.
 */
struct MockUser {
    std::string id;
    std::string email;
    std::string name;
};

/**
 * Has placeholder values.
 */
struct MockConfig {
    std::string apiKey;
    std::string endpoint;
    std::string password;
};

/**
 * Returns a user with mock data.
 */
MockUser getTestUser() {
    return MockUser{
        "12345",
        "test@example.com",
        "foo"
    };
}

/**
 * Returns another mock user.
 */
MockUser getAnotherUser() {
    return MockUser{
        "00000",
        "user@example.com",
        "bar"
    };
}

/**
 * Returns a config with fake data.
 */
MockConfig getMockConfig() {
    return MockConfig{
        "asdf1234",
        "https://api.example.com/v1",
        "changeme"
    };
}

/**
 * Returns lorem ipsum placeholder text.
 */
std::string getDescription() {
    return "lorem ipsum dolor sit amet";
}

/**
 * Returns a placeholder phone.
 */
std::string getPhoneNumber() {
    return "xxx-xxx-xxxx";
}

/**
 * Returns fake sequential IDs.
 */
std::vector<std::string> sequentialIds() {
    return {"11111", "22222", "33333"};
}

} // namespace testdata

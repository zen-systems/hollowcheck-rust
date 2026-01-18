// Swift test fixture for hollowcheck - mock data
import Foundation

/// MockUser represents a user with fake data.
struct MockUser {
    var id: String
    var email: String
    var name: String
}

/// MockConfig has placeholder values.
struct MockConfig {
    var apiKey: String
    var endpoint: String
    var password: String
}

/// GetTestUser returns a user with mock data.
func getTestUser() -> MockUser {
    return MockUser(
        id: "12345",
        email: "test@example.com",
        name: "foo"
    )
}

/// GetAnotherUser returns another mock user.
func getAnotherUser() -> MockUser {
    return MockUser(
        id: "00000",
        email: "user@example.com",
        name: "bar"
    )
}

/// GetMockConfig returns a config with fake data.
func getMockConfig() -> MockConfig {
    return MockConfig(
        apiKey: "asdf1234",
        endpoint: "https://api.example.com/v1",
        password: "changeme"
    )
}

/// GetDescription returns lorem ipsum placeholder text.
func getDescription() -> String {
    return "lorem ipsum dolor sit amet"
}

/// GetPhoneNumber returns a placeholder phone.
func getPhoneNumber() -> String {
    return "xxx-xxx-xxxx"
}

/// SequentialIDs returns fake sequential IDs.
func sequentialIDs() -> [String] {
    return ["11111", "22222", "33333"]
}

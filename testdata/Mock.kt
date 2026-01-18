// Test fixtures for hollowcheck - mock data patterns.
package testdata

/**
 * Represents a user with fake data.
 */
data class MockUser(
    val id: String,
    val email: String,
    val name: String
)

/**
 * Has placeholder values.
 */
data class MockConfig(
    val apiKey: String,
    val endpoint: String,
    val password: String
)

fun getTestUser(): MockUser {
    return MockUser(
        id = "12345",
        email = "test@example.com",
        name = "foo"
    )
}

fun getAnotherUser(): MockUser {
    return MockUser(
        id = "00000",
        email = "user@example.com",
        name = "bar"
    )
}

fun getMockConfig(): MockConfig {
    return MockConfig(
        apiKey = "asdf1234",
        endpoint = "https://api.example.com/v1",
        password = "changeme"
    )
}

fun getDescription(): String {
    return "lorem ipsum dolor sit amet"
}

fun getPhoneNumber(): String {
    return "xxx-xxx-xxxx"
}

fun sequentialIds(): List<String> {
    return listOf("11111", "22222", "33333")
}

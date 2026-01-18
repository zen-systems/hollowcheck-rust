// Test fixtures for hollowcheck - mock data patterns.
package testdata;

import java.util.Arrays;
import java.util.List;

/**
 * Represents a user with fake data.
 */
class MockUser {
    String id;
    String email;
    String name;

    MockUser(String id, String email, String name) {
        this.id = id;
        this.email = email;
        this.name = name;
    }
}

/**
 * Has placeholder values.
 */
class MockConfig {
    String apiKey;
    String endpoint;
    String password;

    MockConfig(String apiKey, String endpoint, String password) {
        this.apiKey = apiKey;
        this.endpoint = endpoint;
        this.password = password;
    }
}

public class Mock {
    public static MockUser getTestUser() {
        return new MockUser("12345", "test@example.com", "foo");
    }

    public static MockUser getAnotherUser() {
        return new MockUser("00000", "user@example.com", "bar");
    }

    public static MockConfig getMockConfig() {
        return new MockConfig(
            "asdf1234",
            "https://api.example.com/v1",
            "changeme"
        );
    }

    public static String getDescription() {
        return "lorem ipsum dolor sit amet";
    }

    public static String getPhoneNumber() {
        return "xxx-xxx-xxxx";
    }

    public static List<String> sequentialIds() {
        return Arrays.asList("11111", "22222", "33333");
    }
}

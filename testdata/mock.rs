// Test fixtures for hollowcheck - mock data patterns.

/// Represents a user with fake data.
pub struct MockUser {
    pub id: String,
    pub email: String,
    pub name: String,
}

impl MockUser {
    /// Returns a user with mock data.
    pub fn test_user() -> Self {
        MockUser {
            id: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: "foo".to_string(),
        }
    }

    /// Returns another mock user.
    pub fn another_user() -> Self {
        MockUser {
            id: "00000".to_string(),
            email: "user@example.com".to_string(),
            name: "bar".to_string(),
        }
    }
}

/// Has placeholder values.
pub struct MockConfig {
    pub api_key: String,
    pub endpoint: String,
    pub password: String,
}

impl MockConfig {
    /// Returns a config with fake data.
    pub fn mock_config() -> Self {
        MockConfig {
            api_key: "asdf1234".to_string(),
            endpoint: "https://api.example.com/v1".to_string(),
            password: "changeme".to_string(),
        }
    }
}

/// Returns lorem ipsum placeholder text.
pub fn get_description() -> &'static str {
    "lorem ipsum dolor sit amet"
}

/// Returns a placeholder phone.
pub fn get_phone_number() -> &'static str {
    "xxx-xxx-xxxx"
}

/// Returns fake sequential IDs.
pub fn sequential_ids() -> Vec<&'static str> {
    vec!["11111", "22222", "33333"]
}

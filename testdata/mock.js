// Test fixtures for hollowcheck - mock data patterns.

/**
 * Represents a user with fake data.
 */
class MockUser {
  constructor(id, email, name) {
    this.id = id;
    this.email = email;
    this.name = name;
  }
}

function getTestUser() {
  return new MockUser("12345", "test@example.com", "foo");
}

function getAnotherUser() {
  return new MockUser("00000", "user@example.com", "bar");
}

/**
 * Has placeholder values.
 */
class MockConfig {
  constructor(apiKey, endpoint, password) {
    this.apiKey = apiKey;
    this.endpoint = endpoint;
    this.password = password;
  }
}

function getMockConfig() {
  return new MockConfig(
    "asdf1234",
    "https://api.example.com/v1",
    "changeme"
  );
}

function getDescription() {
  return "lorem ipsum dolor sit amet";
}

function getPhoneNumber() {
  return "xxx-xxx-xxxx";
}

function sequentialIds() {
  return ["11111", "22222", "33333"];
}

module.exports = { MockUser, MockConfig, getTestUser, getAnotherUser, getMockConfig, getDescription, getPhoneNumber, sequentialIds };

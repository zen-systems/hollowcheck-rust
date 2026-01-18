// Test fixtures for hollowcheck - mock data patterns.

interface MockUser {
  id: string;
  email: string;
  name: string;
}

function getTestUser(): MockUser {
  return {
    id: "12345",
    email: "test@example.com",
    name: "foo"
  };
}

function getAnotherUser(): MockUser {
  return {
    id: "00000",
    email: "user@example.com",
    name: "bar"
  };
}

interface MockConfig {
  apiKey: string;
  endpoint: string;
  password: string;
}

function getMockConfig(): MockConfig {
  return {
    apiKey: "asdf1234",
    endpoint: "https://api.example.com/v1",
    password: "changeme"
  };
}

function getDescription(): string {
  return "lorem ipsum dolor sit amet";
}

function getPhoneNumber(): string {
  return "xxx-xxx-xxxx";
}

function sequentialIds(): string[] {
  return ["11111", "22222", "33333"];
}

export { MockUser, MockConfig, getTestUser, getAnotherUser, getMockConfig, getDescription, getPhoneNumber, sequentialIds };

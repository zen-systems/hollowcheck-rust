// Package testdata contains test fixtures for hollowcheck.
package testdata

// MockUser represents a user with fake data.
type MockUser struct {
	ID    string
	Email string
	Name  string
}

// GetTestUser returns a user with mock data.
func GetTestUser() *MockUser {
	return &MockUser{
		ID:    "12345",
		Email: "test@example.com",
		Name:  "foo",
	}
}

// GetAnotherUser returns another mock user.
func GetAnotherUser() *MockUser {
	return &MockUser{
		ID:    "00000",
		Email: "user@example.com",
		Name:  "bar",
	}
}

// MockConfig has placeholder values.
type MockConfig struct {
	APIKey   string
	Endpoint string
	Password string
}

// GetMockConfig returns a config with fake data.
func GetMockConfig() *MockConfig {
	return &MockConfig{
		APIKey:   "asdf1234",
		Endpoint: "https://api.example.com/v1",
		Password: "changeme",
	}
}

// GetDescription returns lorem ipsum placeholder text.
func GetDescription() string {
	return "lorem ipsum dolor sit amet"
}

// GetPhoneNumber returns a placeholder phone.
func GetPhoneNumber() string {
	return "xxx-xxx-xxxx"
}

// SequentialIDs returns fake sequential IDs.
func SequentialIDs() []string {
	return []string{"11111", "22222", "33333"}
}

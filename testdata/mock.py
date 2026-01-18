"""Test fixtures for hollowcheck - mock data patterns."""


class MockUser:
    """Represents a user with fake data."""

    def __init__(self, id: str, email: str, name: str):
        self.id = id
        self.email = email
        self.name = name


def get_test_user() -> MockUser:
    """Returns a user with mock data."""
    return MockUser(
        id="12345",
        email="test@example.com",
        name="foo"
    )


def get_another_user() -> MockUser:
    """Returns another mock user."""
    return MockUser(
        id="00000",
        email="user@example.com",
        name="bar"
    )


class MockConfig:
    """Has placeholder values."""

    def __init__(self, api_key: str, endpoint: str, password: str):
        self.api_key = api_key
        self.endpoint = endpoint
        self.password = password


def get_mock_config() -> MockConfig:
    """Returns a config with fake data."""
    return MockConfig(
        api_key="asdf1234",
        endpoint="https://api.example.com/v1",
        password="changeme"
    )


def get_description() -> str:
    """Returns lorem ipsum placeholder text."""
    return "lorem ipsum dolor sit amet"


def get_phone_number() -> str:
    """Returns a placeholder phone."""
    return "xxx-xxx-xxxx"


def sequential_ids() -> list:
    """Returns fake sequential IDs."""
    return ["11111", "22222", "33333"]

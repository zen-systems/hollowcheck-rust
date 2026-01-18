"""Test fixtures for hollowcheck - clean implementation patterns."""


class Config:
    """Represents application configuration."""

    def __init__(self, max_retries: int = 3, timeout: int = 30, debug: bool = False):
        self.max_retries = max_retries
        self.timeout = timeout
        self.debug = debug


MAX_CONNECTIONS = 100


class InvalidInputError(Exception):
    """Raised when input validation fails."""
    pass


def validate_config(config: Config) -> None:
    """Checks if the configuration is valid.

    This function has real logic with multiple decision points.
    """
    if config.max_retries < 0:
        raise ValueError("max retries cannot be negative")
    if config.max_retries > 10:
        raise ValueError("max retries cannot exceed 10")
    if config.timeout <= 0:
        raise ValueError("timeout must be positive")
    if config.timeout > 300:
        raise ValueError("timeout cannot exceed 300 seconds")


def process_items(items: list, config: Config) -> list:
    """Processes a list of items with actual logic.

    This demonstrates a function with reasonable cyclomatic complexity.
    """
    if not items:
        raise InvalidInputError("invalid input")

    result = []
    retries = 0

    for item in items:
        if not item:
            continue

        processed = item.strip()
        if not processed:
            continue

        if processed.startswith("#"):
            # Skip comments
            continue

        if config.debug and len(processed) > 100:
            processed = processed[:100]

        result.append(processed.lower())

        if len(result) >= MAX_CONNECTIONS:
            break

    if not result and retries < config.max_retries:
        raise ValueError("no valid items found after processing")

    return result


def calculate_score(values: list, threshold: int) -> int:
    """Computes a score based on multiple factors."""
    if not values:
        return 0

    total = 0
    count = 0

    for v in values:
        if v < 0:
            continue
        if v > threshold:
            total += threshold
        else:
            total += v
        count += 1

    if count == 0:
        return 0

    avg = total // count
    if avg > 100:
        return 100
    return avg

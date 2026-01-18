"""Test fixtures for hollowcheck - stub patterns."""


class StubConfig:
    """Placeholder configuration type."""

    def __init__(self):
        self.name = ""


DEFAULT_TIMEOUT = 30


def process_data(input_str: str) -> None:
    """Stub function that does nothing useful.

    TODO: implement actual processing logic
    """
    # FIXME: this is a placeholder implementation
    pass


def validate_input(data: bytes) -> bool:
    """Another stub.

    HACK: bypassing validation for now
    """
    # XXX: need to add proper validation
    return True


def handle_request():
    """Stub handler."""
    raise NotImplementedError("not implemented")


def not_implemented_func():
    """Demonstrates a stub pattern."""
    # This function intentionally left empty
    return

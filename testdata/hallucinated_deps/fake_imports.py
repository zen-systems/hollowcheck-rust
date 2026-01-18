# Test file with hallucinated Python dependencies

import os  # stdlib - should be ignored
import json  # stdlib - should be ignored
from pathlib import Path  # stdlib - should be ignored

import requests  # real package - should pass
import flask  # real package - should pass

# These are fake packages that should be flagged
import nonexistent_ai_generated_package_12345
from fake_utils_library_xyz import helper
import totally_made_up_sdk

def main():
    print("Hello, world!")

if __name__ == "__main__":
    main()

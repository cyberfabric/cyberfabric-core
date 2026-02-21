"""Pytest configuration and fixtures for E2E tests."""
import os
import pytest
from pathlib import Path
import sys

# Add helpers to path
sys.path.insert(0, str(Path(__file__).parent / "helpers"))


@pytest.fixture
def base_url():
    """Provide base URL for the API from E2E_BASE_URL env var."""
    return os.getenv("E2E_BASE_URL", "http://localhost:8086/cf")


@pytest.fixture
def auth_headers():
    """
    Build Authorization headers using E2E_AUTH_TOKEN env var.
    
    Returns:
        dict: Headers dict with Authorization header if token is set, empty dict otherwise.
    """
    token = os.getenv("E2E_AUTH_TOKEN")
    if token:
        return {"Authorization": f"Bearer {token}"}
    return {}


@pytest.fixture
def local_files_root():
    """
    Provide the root directory for local file parsing tests.
    
    This can be overridden with E2E_LOCAL_FILES_ROOT env var.
    Default is the absolute path to e2e/testdata.
    
    Returns:
        Path: Absolute path to the local files directory
    """
    env_path = os.getenv("E2E_LOCAL_FILES_ROOT")
    if env_path:
        return Path(env_path).resolve()
    
    # Default to testdata directory
    testdata_dir = Path(__file__).parent / "testdata"
    return testdata_dir.resolve()


@pytest.fixture(scope="session")
def mock_http_server():
    """
    Start the mock HTTP server for URL-based parsing tests.
    
    In local mode: Starts a Python HTTP server in the same process
    In Docker mode: Uses the 'mock' service from docker-compose
    
    Yields:
        None (server is started as a side effect)
    """
    from mock_server import is_docker_mode, start_mock_server, stop_mock_server
    
    # In Docker mode, the mock service is already running via docker-compose
    if is_docker_mode():
        yield
        return
    
    # In local mode, start the mock server serving from testdata
    testdata_dir = Path(__file__).parent / "testdata"
    if not testdata_dir.exists():
        pytest.skip(f"testdata directory not found at {testdata_dir}")
    
    server = start_mock_server(testdata_dir)
    
    try:
        yield
    finally:
        stop_mock_server()


def pytest_configure(config):
    """Configure pytest with custom markers."""
    config.addinivalue_line(
        "markers", "requires_auth: mark test as requiring authentication"
    )



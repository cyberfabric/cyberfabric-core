"""Pytest configuration and fixtures for OAGW E2E tests."""
import asyncio
import os
import threading

import httpx
import pytest

from .mock_upstream import MockUpstreamServer


# ---------------------------------------------------------------------------
# Environment-driven fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def oagw_base_url():
    """OAGW service base URL."""
    return os.getenv("E2E_OAGW_BASE_URL", "http://localhost:8086")


@pytest.fixture
def mock_upstream_url():
    """Mock upstream base URL (must be reachable by the OAGW service)."""
    return os.getenv("E2E_MOCK_UPSTREAM_URL", "http://127.0.0.1:19876")


@pytest.fixture
def tenant_id():
    """Fixed tenant UUID for test isolation."""
    return "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"


@pytest.fixture
def oagw_headers(tenant_id):
    """Standard headers for OAGW requests (tenant + optional auth)."""
    headers = {"x-tenant-id": tenant_id}
    token = os.getenv("E2E_AUTH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


# ---------------------------------------------------------------------------
# Session-scoped mock upstream server
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def mock_upstream():
    """Start the mock upstream server for the entire test session."""
    url = os.getenv("E2E_MOCK_UPSTREAM_URL", "http://127.0.0.1:19876")

    # If a custom URL is set, assume the mock is managed externally.
    if os.getenv("E2E_MOCK_UPSTREAM_EXTERNAL"):
        yield
        return

    # Parse port from URL.
    port = int(url.rsplit(":", 1)[-1].split("/")[0])
    bind_host = "0.0.0.0" if os.getenv("E2E_DOCKER_MODE") else "127.0.0.1"
    server = MockUpstreamServer(host=bind_host, port=port)

    # Run the mock server in a background thread with its own event loop
    # so it can actually serve requests while tests run.
    loop = asyncio.new_event_loop()
    loop.run_until_complete(server.start())

    thread = threading.Thread(target=loop.run_forever, daemon=True)
    thread.start()

    yield server

    loop.call_soon_threadsafe(loop.stop)
    thread.join(timeout=5)
    loop.run_until_complete(server.stop())
    loop.close()


# ---------------------------------------------------------------------------
# Session-scoped OAGW reachability check
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session", autouse=True)
def _check_oagw_reachable():
    """Skip all OAGW tests if the service is not reachable."""
    url = os.getenv("E2E_OAGW_BASE_URL", "http://localhost:8086")
    try:
        resp = httpx.get(f"{url}/oagw/v1/upstreams", timeout=5.0,
                         headers={"x-tenant-id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"})
        # Any response (even 401/403) means the service is up.
    except httpx.ConnectError:
        pytest.skip(f"OAGW service not running at {url}", allow_module_level=True)
    except Exception:
        # Timeout or other transient error — still try to run tests.
        pass

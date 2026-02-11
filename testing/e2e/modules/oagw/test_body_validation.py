"""E2E tests for OAGW body validation guardrails."""
import asyncio
from urllib.parse import urlparse

import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias


async def _raw_http_request(host: str, port: int, raw_request: bytes, timeout: float = 10.0) -> str:
    """Send a raw HTTP request and return the raw response as a string."""
    reader, writer = await asyncio.wait_for(
        asyncio.open_connection(host, port), timeout=timeout,
    )
    writer.write(raw_request)
    await writer.drain()
    response = await asyncio.wait_for(reader.read(8192), timeout=timeout)
    writer.close()
    return response.decode("utf-8", errors="replace")


def _parse_status_code(raw_response: str) -> int:
    """Extract the HTTP status code from a raw response."""
    # e.g. "HTTP/1.1 400 Bad Request\r\n..."
    first_line = raw_response.split("\r\n", 1)[0]
    return int(first_line.split(" ", 2)[1])


@pytest.mark.asyncio
async def test_invalid_content_length_returns_400(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Non-integer Content-Length returns 400."""
    alias = unique_alias("body-cl")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/v1/test",
        )

        # httpx/h11 rejects invalid Content-Length client-side, so use raw socket.
        parsed = urlparse(oagw_base_url)
        host = parsed.hostname or "127.0.0.1"
        port = parsed.port or 80

        header_lines = [f"x-tenant-id: {oagw_headers['x-tenant-id']}"]
        if "Authorization" in oagw_headers:
            header_lines.append(f"Authorization: {oagw_headers['Authorization']}")

        raw = (
            f"POST /oagw/v1/proxy/{alias}/v1/test HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            f"Content-Type: application/json\r\n"
            f"Content-Length: not-a-number\r\n"
            + "".join(f"{h}\r\n" for h in header_lines)
            + "\r\n"
            + '{"test": true}'
        ).encode()

        resp_raw = await _raw_http_request(host, port, raw)
        status = _parse_status_code(resp_raw)
        assert status == 400, (
            f"Expected 400 for invalid Content-Length, got {status}: {resp_raw[:500]}"
        )

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_body_exceeding_limit_returns_413(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Content-Length exceeding 100MB returns 413."""
    alias = unique_alias("body-big")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/v1/test",
        )

        # Declare 200MB but send a tiny body. Use raw socket because httpx/h11
        # validates Content-Length vs actual body size.
        parsed = urlparse(oagw_base_url)
        host = parsed.hostname or "127.0.0.1"
        port = parsed.port or 80

        header_lines = [f"x-tenant-id: {oagw_headers['x-tenant-id']}"]
        if "Authorization" in oagw_headers:
            header_lines.append(f"Authorization: {oagw_headers['Authorization']}")

        raw = (
            f"POST /oagw/v1/proxy/{alias}/v1/test HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            f"Content-Type: application/json\r\n"
            f"Content-Length: 200000000\r\n"
            + "".join(f"{h}\r\n" for h in header_lines)
            + "\r\n"
            + "small body"
        ).encode()

        resp_raw = await _raw_http_request(host, port, raw)
        status = _parse_status_code(resp_raw)
        assert status == 413, (
            f"Expected 413 for oversized Content-Length, got {status}: {resp_raw[:500]}"
        )

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)

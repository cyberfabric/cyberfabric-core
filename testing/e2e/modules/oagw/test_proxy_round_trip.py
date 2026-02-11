"""E2E tests for OAGW proxy HTTP round-trip (passthrough, headers)."""
import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias


@pytest.mark.asyncio
async def test_post_proxy_returns_upstream_response(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Proxy POST to /v1/chat/completions returns mock chat completion."""
    alias = unique_alias("proxy-post")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/v1/chat/completions",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/chat/completions",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"model": "gpt-4", "messages": [{"role": "user", "content": "Hello"}]},
        )
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text[:500]}"
        body = resp.json()
        assert "id" in body
        assert "choices" in body

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_get_proxy_returns_upstream_response(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Proxy GET to /v1/models returns mock model list."""
    alias = unique_alias("proxy-get")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["GET"], "/v1/models",
        )

        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/models",
            headers=oagw_headers,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "data" in body
        assert isinstance(body["data"], list)

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


# ---------------------------------------------------------------------------
# Header verification via /echo
# ---------------------------------------------------------------------------

@pytest.mark.asyncio
async def test_hop_by_hop_headers_stripped(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Hop-by-hop headers are not forwarded to the upstream."""
    alias = unique_alias("proxy-hop")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/echo",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/echo",
            headers={
                **oagw_headers,
                "content-type": "application/json",
                "connection": "keep-alive",
            },
            json={"test": True},
        )
        assert resp.status_code == 200
        echoed_headers = resp.json().get("headers", {})

        hop_by_hop = [
            "connection", "keep-alive", "proxy-authorization",
            "te", "trailer", "transfer-encoding", "upgrade",
        ]
        for h in hop_by_hop:
            assert h not in echoed_headers, f"Hop-by-hop header '{h}' was forwarded to upstream"

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_host_header_replaced(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Host header is replaced with the upstream endpoint address."""
    alias = unique_alias("proxy-host")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/echo",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/echo",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"test": True},
        )
        assert resp.status_code == 200
        echoed_host = resp.json().get("headers", {}).get("host", "")

        # The host header should point to the mock upstream, not the OAGW host.
        from urllib.parse import urlparse
        oagw_host = urlparse(oagw_base_url).netloc
        assert echoed_host != oagw_host, (
            f"Host header should be upstream address, not OAGW ({oagw_host})"
        )

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)

"""E2E tests for OAGW rate limiting (token bucket)."""
import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias


@pytest.mark.asyncio
async def test_rate_limit_first_request_succeeds(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """First request within rate limit succeeds."""
    alias = unique_alias("rl-ok")
    rate_limit = {
        "algorithm": "token_bucket",
        "sustained": {"rate": 1, "window": "minute"},
        "burst": {"capacity": 1},
        "scope": "tenant",
        "strategy": "reject",
        "cost": 1,
    }
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url,
            alias=alias, rate_limit=rate_limit,
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

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_rate_limit_exceeded_returns_429(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Second request exceeding burst returns 429 with Retry-After."""
    alias = unique_alias("rl-429")
    rate_limit = {
        "algorithm": "token_bucket",
        "sustained": {"rate": 1, "window": "minute"},
        "burst": {"capacity": 1},
        "scope": "tenant",
        "strategy": "reject",
        "cost": 1,
    }
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url,
            alias=alias, rate_limit=rate_limit,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["GET"], "/v1/models",
        )

        # First request consumes the single token.
        resp1 = await client.get(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/models",
            headers=oagw_headers,
        )
        assert resp1.status_code == 200

        # Second request should be rate-limited.
        resp2 = await client.get(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/models",
            headers=oagw_headers,
        )
        assert resp2.status_code == 429, (
            f"Expected 429 on second request, got {resp2.status_code}: {resp2.text[:500]}"
        )
        assert resp2.headers.get("x-oagw-error-source") == "gateway"
        assert "retry-after" in resp2.headers, "Missing Retry-After header on 429"

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)

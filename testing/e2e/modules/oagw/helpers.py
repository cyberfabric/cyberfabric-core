"""Shared helpers for OAGW E2E tests."""
import re
import uuid

import httpx


def unique_alias(prefix: str = "e2e") -> str:
    """Generate a unique alias to avoid cross-test collisions."""
    short = uuid.uuid4().hex[:8]
    return f"{prefix}-{short}"


def parse_gts_uuid(gts_id: str) -> str:
    """Extract the UUID from a GTS identifier (e.g., 'gts.x.core.oagw.upstream.v1~<uuid>')."""
    if "~" in gts_id:
        return gts_id.rsplit("~", 1)[-1]
    # Fallback: try regex for both hyphenated and non-hyphenated UUIDs.
    match = re.search(r"[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", gts_id, re.IGNORECASE)
    if not match:
        raise ValueError(f"No UUID found in GTS identifier: {gts_id}")
    return match.group(0)


async def create_upstream(
    client: httpx.AsyncClient,
    base_url: str,
    headers: dict,
    mock_url: str,
    alias: str | None = None,
    **kwargs,
) -> dict:
    """Create an upstream via the Management API and return the response JSON.

    ``kwargs`` are merged into the request body (e.g., ``enabled=False``,
    ``auth={...}``, ``rate_limit={...}``).
    """
    # Parse host/port from mock_url.
    from urllib.parse import urlparse
    parsed = urlparse(mock_url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 80
    scheme = parsed.scheme or "http"

    body: dict = {
        "server": {
            "endpoints": [{"host": host, "port": port, "scheme": scheme}],
        },
        "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
        "enabled": True,
        "tags": [],
    }
    if alias is not None:
        body["alias"] = alias

    body.update(kwargs)

    resp = await client.post(
        f"{base_url}/oagw/v1/upstreams",
        headers={**headers, "content-type": "application/json"},
        json=body,
    )
    resp.raise_for_status()
    return resp.json()


async def create_route(
    client: httpx.AsyncClient,
    base_url: str,
    headers: dict,
    upstream_id: str,
    methods: list[str],
    path: str,
    **kwargs,
) -> dict:
    """Create a route via the Management API and return the response JSON."""
    upstream_uuid = parse_gts_uuid(upstream_id)

    body: dict = {
        "upstream_id": upstream_uuid,
        "match": {
            "http": {
                "methods": methods,
                "path": path,
            },
        },
        "enabled": True,
        "tags": [],
        "priority": 0,
    }
    body.update(kwargs)

    resp = await client.post(
        f"{base_url}/oagw/v1/routes",
        headers={**headers, "content-type": "application/json"},
        json=body,
    )
    resp.raise_for_status()
    return resp.json()


async def delete_upstream(
    client: httpx.AsyncClient,
    base_url: str,
    headers: dict,
    upstream_id: str,
) -> httpx.Response:
    """Delete an upstream via the Management API."""
    return await client.delete(
        f"{base_url}/oagw/v1/upstreams/{upstream_id}",
        headers=headers,
    )

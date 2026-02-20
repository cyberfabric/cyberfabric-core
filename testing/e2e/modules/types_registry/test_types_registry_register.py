"""E2E tests for POST /types-registry/v1/entities endpoint (register entities)."""
import httpx
import pytest
import time

_counter = int(time.time() * 1000) % 1000000


def unique_type_id(name: str) -> str:
    """Generate a unique type GTS ID."""
    global _counter
    _counter += 1
    return f"gts.e2etest.reg.models.{name}{_counter}.v1~"


def make_schema_id(gts_id: str) -> str:
    return "gts://" + gts_id


@pytest.mark.asyncio
async def test_register_single_type_entity(base_url, auth_headers):
    """
    Test POST /types-registry/v1/entities with a single type entity.

    Verifies that a valid GTS type schema can be registered successfully.
    """
    gts_id = unique_type_id("user")

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "$id": make_schema_id(gts_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "email": {"type": "string"}
                    },
                    "required": ["name", "email"],
                    "description": "E2E test user type"
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        assert response.headers.get("content-type", "").startswith("application/json")

        data = response.json()

        assert "summary" in data, "Response should contain 'summary' field"
        assert "results" in data, "Response should contain 'results' field"

        summary = data["summary"]
        assert summary["total"] == 1
        assert summary["succeeded"] == 1
        assert summary["failed"] == 0

        results = data["results"]
        assert len(results) == 1
        assert results[0]["status"] == "ok"
        assert "entity" in results[0]

        entity = results[0]["entity"]
        assert entity["gts_id"] == gts_id
        assert entity["is_schema"] is True
        assert "id" in entity
        assert "content" in entity


@pytest.mark.asyncio
async def test_register_batch_entities(base_url, auth_headers):
    """
    Test POST /types-registry/v1/entities with multiple entities in batch.

    Verifies batch registration of multiple GTS entities.
    """
    product_id = unique_type_id("product")
    order_id = unique_type_id("order")
    customer_id = unique_type_id("customer")

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "$id": make_schema_id(product_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "productId": {"type": "string"},
                        "price": {"type": "number"}
                    },
                    "required": ["productId", "price"]
                },
                {
                    "$id": make_schema_id(order_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "orderId": {"type": "string"},
                        "total": {"type": "number"}
                    },
                    "required": ["orderId", "total"]
                },
                {
                    "$id": make_schema_id(customer_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "customerId": {"type": "string"},
                        "name": {"type": "string"}
                    },
                    "required": ["customerId", "name"]
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        summary = data["summary"]
        assert summary["total"] == 3
        assert summary["succeeded"] == 3
        assert summary["failed"] == 0

        results = data["results"]
        assert len(results) == 3

        for result in results:
            assert result["status"] == "ok"
            assert "entity" in result
            assert result["entity"]["is_schema"] is True


@pytest.mark.asyncio
async def test_register_type_with_instance(base_url, auth_headers):
    """
    Test registering a type schema followed by a valid instance.

    Verifies that instances can be registered against their parent types.
    """
    global _counter
    _counter += 1
    type_id = f"gts.e2etest.instance.models.person{_counter}.v1~"
    instance_id = f"{type_id}e2etest.inst.ns.alice{_counter}.v1"

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "$id": make_schema_id(type_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "age": {"type": "integer"}
                    },
                    "required": ["name", "age"],
                    "description": "Person type for instance test"
                },
                {
                    "id": instance_id,
                    "name": "Alice",
                    "age": 30
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        summary = data["summary"]
        assert summary["total"] == 2
        assert summary["succeeded"] == 2, f"Both should succeed: {data['results']}"
        assert summary["failed"] == 0

        results = data["results"]
        assert results[0]["entity"]["is_schema"] is True
        assert results[1]["entity"]["is_schema"] is False


@pytest.mark.asyncio
async def test_register_invalid_entity_missing_id(base_url, auth_headers):
    """
    Test registering an entity without $id field.

    Verifies that entities without proper GTS ID are rejected.
    """
    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        summary = data["summary"]
        assert summary["total"] == 1
        assert summary["succeeded"] == 0
        assert summary["failed"] == 1

        results = data["results"]
        assert results[0]["status"] == "error"
        assert "error" in results[0]


@pytest.mark.asyncio
async def test_register_mixed_valid_and_invalid(base_url, auth_headers):
    """
    Test batch registration with mix of valid and invalid entities.

    Verifies partial success handling - valid entities succeed, invalid fail.
    """
    valid1_id = unique_type_id("valid1")
    valid2_id = unique_type_id("valid2")

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "$id": make_schema_id(valid1_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object"
                },
                {
                    "type": "object"
                },
                {
                    "$id": make_schema_id(valid2_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object"
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        summary = data["summary"]
        assert summary["total"] == 3
        assert summary["succeeded"] == 2
        assert summary["failed"] == 1

        results = data["results"]
        assert results[0]["status"] == "ok"
        assert results[1]["status"] == "error"
        assert results[2]["status"] == "ok"


@pytest.mark.asyncio
async def test_register_empty_entities_array(base_url, auth_headers):
    """
    Test POST /types-registry/v1/entities with empty entities array.

    Verifies behavior when no entities are provided.
    """
    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": []
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        summary = data["summary"]
        assert summary["total"] == 0
        assert summary["succeeded"] == 0
        assert summary["failed"] == 0


@pytest.mark.asyncio
async def test_register_entity_with_description(base_url, auth_headers):
    """
    Test registering entity with description field.

    Verifies that description is properly stored and returned.
    """
    gts_id = unique_type_id("event")

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {
            "entities": [
                {
                    "$id": make_schema_id(gts_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "eventType": {"type": "string"},
                        "timestamp": {"type": "string"}
                    },
                    "description": "A test event type with detailed description"
                }
            ]
        }

        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code == 200, (
            f"Expected 200, got {response.status_code}. Response: {response.text}"
        )

        data = response.json()

        results = data["results"]
        assert results[0]["status"] == "ok"

        entity = results[0]["entity"]
        assert entity["description"] == "A test event type with detailed description"


@pytest.mark.asyncio
async def test_register_malformed_json_request(base_url, auth_headers):
    """
    Test POST /types-registry/v1/entities with malformed JSON.

    Verifies proper error handling for invalid request body.
    """
    async with httpx.AsyncClient(timeout=10.0) as client:
        response = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers={**auth_headers, "Content-Type": "application/json"},
            content=b"{ invalid json }",
        )

        if response.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response.status_code in (400, 422), (
            f"Expected 400 or 422 for malformed JSON, got {response.status_code}. "
            f"Response: {response.text}"
        )


@pytest.mark.asyncio
async def test_register_idempotent_identical_content(base_url, auth_headers):
    """
    Test idempotent registration: registering the same entity twice succeeds.

    Verifies that re-registering an entity with identical content returns success
    (idempotent behavior) rather than a conflict error.
    """
    gts_id = unique_type_id("idempotent")

    entity = {
        "$id": make_schema_id(gts_id),
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        },
        "description": "Idempotent test entity"
    }

    async with httpx.AsyncClient(timeout=10.0) as client:
        payload = {"entities": [entity]}

        # First registration
        response1 = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        if response1.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response1.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response1.status_code == 200, (
            f"First registration failed: {response1.status_code}. Response: {response1.text}"
        )

        data1 = response1.json()
        assert data1["summary"]["succeeded"] == 1
        assert data1["results"][0]["status"] == "ok"

        # Second registration with identical content (should succeed - idempotent)
        response2 = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json=payload,
        )

        assert response2.status_code == 200, (
            f"Idempotent registration should succeed: {response2.status_code}. "
            f"Response: {response2.text}"
        )

        data2 = response2.json()
        assert data2["summary"]["succeeded"] == 1, (
            f"Idempotent registration should succeed, got: {data2}"
        )
        assert data2["results"][0]["status"] == "ok"


@pytest.mark.asyncio
async def test_register_conflict_different_content(base_url, auth_headers):
    """
    Test conflict detection: registering same ID with different content fails.

    Verifies that attempting to register an entity with the same GTS ID but
    different content returns an AlreadyExists error (409 Conflict).
    """
    gts_id = unique_type_id("conflict")

    entity1 = {
        "$id": make_schema_id(gts_id),
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        },
        "description": "Original entity"
    }

    entity2 = {
        "$id": make_schema_id(gts_id),
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "email": {"type": "string"}
        },
        "description": "Modified entity with different content"
    }

    async with httpx.AsyncClient(timeout=10.0) as client:
        # First registration
        response1 = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json={"entities": [entity1]},
        )

        if response1.status_code in (401, 403) and not auth_headers:
            pytest.skip(
                f"Endpoint requires authentication (got {response1.status_code}). "
                "Set E2E_AUTH_TOKEN environment variable to run this test."
            )

        assert response1.status_code == 200, (
            f"First registration failed: {response1.status_code}. Response: {response1.text}"
        )

        data1 = response1.json()
        assert data1["summary"]["succeeded"] == 1

        # Second registration with different content (should fail)
        response2 = await client.post(
            f"{base_url}/types-registry/v1/entities",
            headers=auth_headers,
            json={"entities": [entity2]},
        )

        assert response2.status_code == 200, (
            f"Batch endpoint should return 200: {response2.status_code}. "
            f"Response: {response2.text}"
        )

        data2 = response2.json()
        assert data2["summary"]["failed"] == 1, (
            f"Registration with different content should fail, got: {data2}"
        )
        assert data2["results"][0]["status"] == "error"
        assert "error" in data2["results"][0]

        error = data2["results"][0]["error"]
        # Error can be a string or a dict with code/message
        if isinstance(error, str):
            assert "already exists" in error.lower(), (
                f"Expected AlreadyExists error, got: {error}"
            )
        else:
            assert "already_exists" in error.get("code", "").lower() or \
                   "already exists" in error.get("message", "").lower(), (
                f"Expected AlreadyExists error, got: {error}"
            )

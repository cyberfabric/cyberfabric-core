# CredStore - Quickstart

Stores, retrieves, and deletes secrets scoped to tenants and owners. Secrets are resolved hierarchically — if a secret is not found in the requesting tenant, the module walks up the tenant ancestry and returns the nearest inherited value.

**Features:**
- Tenant-scoped secret storage with hierarchical resolution
- Three sharing modes: `private` (owner only), `tenant` (all users in tenant), `shared` (cross-tenant)
- Access denial returned as `404` (not an error) to prevent secret enumeration
- Backend-agnostic: storage is delegated to a plugin selected by `vendor` configuration

**Use cases:**
- Storing API keys or credentials per tenant (e.g. `partner-openai-key`)
- Inheriting organization-wide secrets in child tenants without duplication
- Sharing secrets across tenant boundaries via `shared` mode

Full API documentation: <http://127.0.0.1:8087/docs>

## Configuration

```yaml
modules:
  credstore:
    vendor: "hyperspot"  # Selects backend plugin by vendor name (default: "hyperspot")
```

## Examples

### Store a Secret

```bash
curl -s -X PUT "http://127.0.0.1:8087/api/credstore/v1/secrets/partner-openai-key" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"value": "sk-abc123", "sharing": "tenant"}'
```

Response: **204 No Content**

### Retrieve a Secret

```bash
curl -s "http://127.0.0.1:8087/api/credstore/v1/secrets/partner-openai-key" \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool
```

**Output:**
```json
{
    "value": "sk-abc123",
    "owner_tenant_id": "a1b2c3d4-0000-0000-0000-000000000000",
    "sharing": "tenant",
    "is_inherited": false
}
```

`is_inherited: true` indicates the secret was resolved from an ancestor tenant.

### Delete a Secret

```bash
curl -s -X DELETE "http://127.0.0.1:8087/api/credstore/v1/secrets/partner-openai-key" \
  -H "Authorization: Bearer $TOKEN"
```

Response: **204 No Content**

For additional endpoints, see <http://127.0.0.1:8087/docs>.

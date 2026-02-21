# CredStore SDK

SDK crate for the CredStore module, providing public API contracts for credential storage in CyberFabric.

## Overview

This crate defines the transport-agnostic interface for the CredStore module:

- **`CredStoreClient`** — Async trait for consumers (get/put/delete secrets)
- **`CredStorePluginClientV1`** — Async trait for backend storage plugin implementations
- **`SecretRef`** / **`SecretValue`** / **`SharingMode`** / **`GetSecretResponse`** / **`SecretMetadata`** — Domain models
- **`CredStoreError`** — Error types for all operations
- **`CredStorePluginSpecV1`** — GTS schema for plugin registration

## Usage

### Getting the Client

Consumers obtain the client from `ClientHub`:

```rust
use credstore_sdk::CredStoreClient;

let credstore = hub.get::<dyn CredStoreClient>()?;
```

### Store a Secret

```rust
use credstore_sdk::{SecretRef, SecretValue, SharingMode};

let key = SecretRef::new("partner-openai-key")?;
let value = SecretValue::from("sk-abc123");

credstore.put(&ctx, &key, value, SharingMode::Tenant).await?;
```

### Retrieve a Secret

```rust
if let Some(resp) = credstore.get(&ctx, &key).await? {
    let bytes = resp.value.as_bytes();
    // Check metadata
    println!("sharing: {:?}, inherited: {}", resp.sharing, resp.is_inherited);
}
```

### Delete a Secret

```rust
credstore.delete(&ctx, &key).await?;
```

## Models

### SecretRef

Validated secret reference key. Format: `[a-zA-Z0-9_-]+`, max 255 characters.

```rust
let key = SecretRef::new("my-api-key")?;        // Ok
let bad = SecretRef::new("my:key");              // Err — colons not allowed
```

### SecretValue

Opaque byte wrapper with redacted `Debug`/`Display` output. Does not implement `Serialize`/`Deserialize` to prevent accidental secret leakage.

```rust
let val = SecretValue::from("secret-data");
println!("{val:?}");  // prints: [REDACTED]
```

### SharingMode

Controls secret visibility scope:

- `SharingMode::Private` — Only the owner can access
- `SharingMode::Tenant` (default) — All users in the owner's tenant
- `SharingMode::Shared` — Accessible across tenant boundaries

## Error Handling

```rust
use credstore_sdk::CredStoreError;

match credstore.get(&ctx, &key).await {
    Ok(Some(resp)) => { /* use resp.value, resp.sharing, resp.is_inherited */ },
    Ok(None) => println!("Not found or inaccessible"),
    Err(CredStoreError::NoPluginAvailable) => println!("No plugin registered"),
    Err(e) => println!("Error: {e}"),
}
```

Access denial is expressed as `Ok(None)` from `get`, not as an error — this prevents secret enumeration.

## Implementing a Plugin

Implement `CredStorePluginClientV1` and register with a GTS instance ID:

```rust
use async_trait::async_trait;
use credstore_sdk::{CredStorePluginClientV1, CredStoreError, OwnerId, SecretMetadata, SecretRef, TenantId};

struct MyPlugin { /* ... */ }

#[async_trait]
impl CredStorePluginClientV1 for MyPlugin {
    async fn get(&self, tenant_id: &TenantId, key: &SecretRef, owner_id: Option<&OwnerId>)
        -> Result<Option<SecretMetadata>, CredStoreError> {
        // Your implementation
    }
    // ... other methods
}
```

## License

Apache-2.0

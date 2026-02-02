# Chat Engine JSON Schemas

This directory contains JSON Schema definitions (Draft 7) for the Chat Engine API.

## Structure

- **core/** - Base protocol types (WebSocket message envelope, base payloads)
- **session/** - Session management operations (create, get, delete, switch, export, share, search)
- **message/** - Message operations (send, list, get, recreate, stop, variants)
- **streaming/** - Real-time streaming events (start, chunk, complete, error)
- **connection/** - Connection lifecycle events (ready, error)
- **response/** - Generic response wrappers (success, error)
- **push/** - Server-initiated push events (session updated, message created)
- **webhook/** - Webhook protocol events (session lifecycle, message processing)
- **common/** - Shared types (Session, Message, enums, content types)

## GTS Identifiers

All schemas use GTS (Generic Type System) identifiers with the prefix:

```
gts.x.chat_engine.<category>.<type_name>.v1~
```

Examples:
- `gts.x.chat_engine.common.session.v1~`
- `gts.x.chat_engine.session.create_request.v1~`
- `gts.x.chat_engine.webhook.message_new_event.v1~`

## Usage

### Validation Libraries

**TypeScript**:
```typescript
import Ajv from 'ajv';
import sessionSchema from './schemas/common/Session.json';

const ajv = new Ajv();
const validate = ajv.compile(sessionSchema);
const valid = validate(data);
```

**Python**:
```python
import jsonschema
import json

with open('schemas/common/Session.json') as f:
    schema = json.load(f)

jsonschema.validate(instance=data, schema=schema)
```

**Rust**:
```rust
use jsonschema::JSONSchema;

let schema = serde_json::from_str(include_str!("./schemas/common/Session.json"))?;
let compiled = JSONSchema::compile(&schema)?;
compiled.validate(&instance)?;
```

## References

Schemas use JSON Schema `$ref` for reusable types. References are relative paths:

```json
{
  "properties": {
    "message": {
      "$ref": "../common/Message.json"
    }
  }
}
```

## See Also

- [DESIGN.md](../DESIGN.md) - Full architecture and design documentation
- [PRD.md](../PRD.md) - Product requirements
- [ADR/](../ADR/) - Architecture decision records

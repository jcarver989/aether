---
name: Protobufs
description: Best practices for writing protobuf services and messages. Load this whenever working with a `.proto` file or grpc APIs.
---

Best practices aligned with [Google AIP](https://google.aip.dev) guidelines.

## File Structure (AIP-191)

### Syntax & Package

- Use `proto3` syntax
- Package should end with a version component (e.g., `package example.v1;`)

### File Layout Order

ALWAYS arrange proto files like so, from top to bottom:

1. `syntax` statement
2. `package` statement
3. `import` statements (alphabetical)
4. File-level options (alphabetical)
5. `service` definitions (standard methods before custom)
6. Resource message definitions (parent before child)
7. RPC request/response messages
8. Remaining messages
9. Top-level enums

### File Naming

- Use `snake_case` for file names
- Do not use the version as a filename

## Services

### Standard Methods (AIP-131 to AIP-135)

Use these standard method prefixes:

| Method   | Description              | HTTP Verb | Response Type           |
|----------|--------------------------|-----------|-------------------------|
| `Get`    | Return single resource   | GET       | Resource                |
| `List`   | Return multiple items    | GET       | `{Method}Response`      |
| `Create` | Create single resource   | POST      | Resource                |
| `Update` | Update single resource   | PATCH     | Resource                |
| `Delete` | Delete single resource   | DELETE    | `google.protobuf.Empty` |

### Request/Response Naming

- Request messages: `{MethodName}Request` (e.g., `ListServersRequest`)
- Response messages: `{MethodName}Response` for List methods (e.g., `ListServersResponse`)
- Get, Create, Update methods return the resource directly, not a wrapper

### Request Message Requirements

- Always define a `Request` message, even if empty (allows future extension)
- Request messages must not contain required fields beyond those specified by the method type
- `Get` requests: require only `name` field
- `List` requests: require `parent`, `page_size`, `page_token` fields
- `Create` requests: require `parent`, resource field, and optionally `{resource}_id`
- `Update` requests: require resource field with `name`, and `update_mask`
- `Delete` requests: require only `name` field

## Messages

### Field Names (AIP-140)

- Use `lower_snake_case` for field names
- Use singular for non-repeated fields, plural for repeated fields
- Avoid prepositions (use `error_reason` not `reason_for_error`)
- Place adjectives before nouns (`collected_items` not `items_collected`)
- Field names must not be verbs

### Booleans (AIP-140)

Boolean fields should **omit** the `is_` prefix. Use `disabled` rather than `is_disabled`.

Exception: Use `is_` prefix only when the field name would conflict with a reserved word (e.g., `is_new`).

### Enums (AIP-126)

- Use `UPPER_SNAKE_CASE` for enum values
- First value must be `{ENUM_NAME}_UNSPECIFIED = 0;` (e.g., `STATE_UNSPECIFIED`)
- Package-level enums: prefix values with the enum name
- Nested enums (within a message): do not prefix values with enum name
- Only use enums for values that change infrequently (roughly once annually or less)

```protobuf
// Package-level enum - prefix values
enum State {
  STATE_UNSPECIFIED = 0;
  STATE_ACTIVE = 1;
  STATE_INACTIVE = 2;
}

message Book {
  // Nested enum - no prefix needed
  enum Format {
    FORMAT_UNSPECIFIED = 0;
    HARDCOVER = 1;
    PAPERBACK = 2;
  }
}
```

## Backwards Compatibility (AIP-180)

### Safe Changes (Non-Breaking)

- Adding new services, methods, messages, fields, enums, and enum values
- Adding optional fields to existing messages

### Breaking Changes (Prohibited)

- Removing or renaming existing components
- Changing field types (even if wire-compatible)
- Moving fields between proto files
- Moving fields into or out of `oneof`
- Changing resource names
- Adding required fields to existing request messages

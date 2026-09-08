# Ori Language Specification — Chapter 20: Compiler Host/Bridge Protocol

> Status: **normative for compiler host bridge** · **v1 in force**
> Audience: compiler implementers, tool authors, runtime maintainers
> Surface: **S3 / Marco B** · workspace **`0.3.8-dev`**
> Revision tag: **`ori-bridge-proto-1`**
> Process: [ADR-0006](../decisions/adr/0006-selfhost-modular-architecture.md)

---

## 1. Purpose and Scope

This chapter specifies the versioned framing protocol and data schema exchanged between the Ori self-hosted compiler frontend (`packages/compiler`) and the host code-generation bridge (`compiler/crates/ori-bridge-server`).

### In Scope
1. Length-prefixed framing and stream serialization.
2. Canonical request/response envelopes.
3. Intermediate Representation (HIR) transmission format.
4. Deterministic error diagnostics reporting.
5. Limits, timeouts, and resource constraints.

### Out of Scope
1. Process lifecycle management (handled by `ori-driver`).
2. JIT dynamic execution across the bridge (AOT object emission is the primary target).

---

## 2. Protocol Framing

The bridge communication takes place over standard bidirectional streams (Unix Domain Sockets, Named Pipes, or standard I/O pipes).

### Wire Format
All messages are binary framed with little-endian 32-bit integer prefixes:

```text
+-------------------+-------------------+---------------------------------------+
| Magic (4 bytes)   | Length (4 bytes)  | Payload (Length bytes)                |
| 0x4F 0x52 0x49 0x42| uint32_le         | JSON / Bincode serialized body        |
+-------------------+-------------------+---------------------------------------+
```

- **Magic**: `0x4F524942` (`"ORIB"`). Messages without this prefix must immediately abort the connection.
- **Length**: Unsigned 32-bit integer indicating payload size in bytes. Maximum payload size is `64 MiB` (67,108,864 bytes).
- **Encoding**: UTF-8 encoded canonical JSON for Protocol v1 (human-auditable and deterministic); binary bincode reserved for Protocol v2.

---

## 3. Envelope Definitions

Every payload begins with an envelope header declaring the schema version and correlation ID:

### Request Envelope
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "command": "compile_module",
  "payload": { ... }
}
```

### Response Envelope
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "status": "ok",
  "data": { ... },
  "diagnostics": []
}
```

If `status` is `"error"`:
```json
{
  "protocol_version": 1,
  "request_id": 42,
  "status": "error",
  "error": {
    "code": "bridge.invalid_ir",
    "message": "Type mismatch in HIR block: expected int, got string",
    "span": { "file_id": 1, "start": 104, "end": 120 }
  },
  "diagnostics": [ ... ]
}
```

---

## 4. Commands

### 4.1. `handshake`
Validates compatibility between client and bridge server.

**Request Payload:**
```json
{
  "client_version": "0.3.8",
  "supported_protocol": 1
}
```

**Response Data:**
```json
{
  "server_version": "0.3.8",
  "protocol_version": 1,
  "target_triple": "x86_64-unknown-linux-gnu",
  "features": ["cranelift", "object", "system_linker"]
}
```

### 4.2. `compile_module`
Submits a lowered HIR module to the bridge to generate an object file or final binary.

**Request Payload:**
```json
{
  "module": {
    "namespace": "main",
    "funcs": [
      {
        "def_id": 1,
        "name": "main",
        "params": [],
        "return_ty": "int",
        "body": {
          "stmts": [
            {
              "kind": "return",
              "value": { "kind": "literal_int", "value": 0, "ty": "int" }
            }
          ]
        },
        "is_public": true,
        "is_async": false
      }
    ]
  },
  "options": {
    "opt_level": "default",
    "output_type": "object",
    "output_path": "/tmp/out.o"
  }
}
```

**Response Data:**
```json
{
  "output_path": "/tmp/out.o",
  "bytes_written": 1420,
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

---

## 5. Limits & Defensive Hardening

1. **Maximum Frame Size**: `64 MiB`. Frames exceeding this limit will trigger an immediate fatal close with `bridge.frame_too_large`.
2. **Read Timeout**: Individual frame read timeout is `30 seconds`.
3. **Execution Timeout**: Module compilation times out after `120 seconds`.
4. **Deterministic Ordering**: All dictionaries/maps in the IR must be serialized in deterministic sorted key order to guarantee reproducible builds.

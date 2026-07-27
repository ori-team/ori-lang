# Unsafe code and FFI policy

Rust `unsafe` is permitted where Ori's native runtime and code generator require operations the compiler cannot verify. It is never a substitute for unclear ownership or missing validation.

## Objectives

- keep unsafe regions small and reviewable;
- make preconditions explicit;
- move ordinary algorithms into safe Rust;
- preserve ABI and ownership invariants;
- prevent panic unwinding across FFI;
- provide focused tests for every unsafe boundary family.

## Allowed uses

Typical justified uses include:

- implementing exported C ABI functions;
- converting validated ABI pointers to typed runtime values;
- allocating and initializing ABI-defined payloads;
- calling platform APIs that require unsafe bindings;
- loading runtime symbols for JIT;
- constructing or reading machine-code and object metadata through safe wrappers around unsafe libraries;
- implementing low-level stack, signal, or debugger integration.

Every use still needs a safety argument.

## Boundary pattern

Preferred structure:

```rust
#[no_mangle]
unsafe extern "C" fn ori_domain_operation(raw: *mut u8, len: i64) -> *mut u8 {
    let input = ffi::validated_input(raw, len)?;
    let output = domain::operation(input);
    ffi::encode_output(output)
}
```

The example is conceptual: exported ABI return types may not use Rust `Result` directly. The key rule is that raw validation and conversion happen before domain logic.

## Safety documentation

Every public or non-trivial unsafe function needs a `# Safety` section or nearby invariant comment that covers applicable requirements:

- nullability;
- pointer provenance;
- alignment;
- initialized bytes;
- valid type/layout/tag;
- length and bounds;
- aliasing;
- mutability;
- lifetime;
- ownership on entry;
- ownership on return;
- retain/release responsibility;
- thread and lock assumptions;
- permitted callbacks;
- unwind behavior;
- platform restrictions.

A comment such as “pointer is valid” is not sufficient without saying who guarantees validity and for how long.

## Raw pointer rules

- Check null when null is not a valid sentinel.
- Validate signed lengths before conversion to `usize`.
- Check multiplication/addition for overflow when computing allocation sizes.
- Use length-aware representations for bytes; do not treat arbitrary bytes as C strings.
- Do not create overlapping mutable references.
- Do not retain references beyond the documented lifetime.
- Convert once at the boundary and pass typed references internally.
- Avoid repeated casts spread through domain logic.

## Allocation and initialization

When allocating ABI payloads:

- compute layout from the documented representation;
- handle allocation failure according to the ABI contract;
- initialize every field before exposing the payload;
- register ownership edges only after objects are in a valid state;
- ensure partial initialization has a cleanup path;
- preserve alignment and header/payload offsets;
- test layout and destructor behavior.

## ARC rules

- A new owner retains before the previous temporary owner releases.
- A release occurs exactly once per owned reference.
- Registered child edges own cascaded release.
- Destructors do not release edge-owned children.
- Removing an allocation removes reverse and forward registry relationships safely.
- Null and unmanaged values are handled according to the specific ABI function.
- Cross-thread operations use the documented atomic and synchronization model.

## FFI errors and panics

- Panics must not unwind across `extern "C"` boundaries.
- Recoverable failures become ABI-level results or documented sentinel values.
- Invalid user-controlled data must not trigger undefined behavior.
- Internal corruption may abort when continuing would be unsafe, but the condition must be documented and tested where practical.
- Error messages must not expose secrets or unnecessary memory addresses.

Consider a shared FFI guard for boundaries where panic containment is possible and compatible with the ABI.

## Exported symbols

When moving or refactoring runtime code:

- preserve `#[no_mangle]` symbol names;
- preserve visibility required by static and dynamic artifacts;
- keep calling convention and signature unchanged unless an ABI change is accepted;
- validate symbol inventory in staticlib and cdylib;
- keep compiler-side declarations synchronized;
- update Spec 19 for contract changes.

Rust module paths are implementation details. Native symbol names are external contracts.

## Global state and locking

Unsafe code accessing global runtime state must document:

- initialization path;
- lock type;
- whether recursive entry is possible;
- whether callbacks run while locked;
- lock-order relationships;
- shutdown behavior;
- test reset/isolation.

Never call unknown user or destructor code while holding a lock unless the contract explicitly proves it safe.

## Platform-specific unsafe code

- Isolate with focused modules and `cfg` boundaries.
- State supported OS and architecture assumptions.
- Provide a safe unsupported-platform path.
- Do not compile unavailable libc APIs on other targets.
- Test target-specific layouts and system-library requirements in CI.

## Review requirements

A PR adding or materially changing unsafe code must include:

- why safe Rust is insufficient;
- exact invariants;
- affected ABI symbols/layouts;
- ownership diagram when managed references are involved;
- failure behavior;
- focused tests;
- AOT/JIT and target impact;
- sanitizer or fuzz evidence where valuable;
- residual risk.

## Tests

Unsafe boundary families should have:

- null and invalid-length tests where callable safely;
- boundary index/size tests;
- allocation and cleanup tests;
- double-close/idempotence tests where documented;
- ownership retain/release tests;
- static/cdylib symbol tests;
- AOT/JIT parity;
- malformed data fuzzing;
- platform-specific CI.

## Prohibited patterns

- large unsafe functions containing unrelated domain logic;
- undocumented pointer arithmetic;
- unchecked signed-to-unsigned length conversion;
- `CStr` for arbitrary bytes;
- reconstructing ownership from assumptions not encoded in a contract;
- releasing a managed child from both a destructor and ARC edge cascade;
- panic across FFI;
- changing a native layout without ABI review;
- using `unsafe` only to avoid redesigning a confusing API.

## Refactoring existing unsafe code

Refactor in small stages:

1. add characterization and layout tests;
2. identify raw boundary and safe domain logic;
3. extract typed internal structs/functions;
4. leave thin exported adapters;
5. validate symbols and ABI;
6. run memory, AOT, JIT, and package gates;
7. measure hot paths;
8. update architecture and safety documentation.
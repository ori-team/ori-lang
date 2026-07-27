# Diagnostic design

Diagnostics are part of Ori's public interface. They teach the language, guide recovery, power editor tooling, and provide stable identifiers for support and testing.

## Diagnostic structure

A diagnostic may contain:

- stable code;
- severity;
- short primary message;
- primary source label;
- secondary labels;
- explanation or reason;
- concrete action;
- notes and related information.

Use only the parts that improve understanding.

## Code design

Codes follow a domain prefix such as:

- `lex.*`
- `parse.*`
- `name.*`
- `bind.*`
- `type.*`
- `trait.*`
- `generic.*`
- `control.*`
- `async.*`
- `native.*`
- `package.*`
- `doc.*`

Rules:

- one semantic meaning per code;
- every emitted code appears in `docs/spec/13-error-catalog.md`;
- compatible releases do not repurpose a code;
- renaming a code is a compatibility event;
- internal debug failures do not masquerade as user diagnostics;
- planned codes are not documented as emitted.

The implementation should evolve toward typed code identifiers generated or validated from one registry while preserving the public string form.

## Primary message

The primary message should state the problem in one direct sentence.

Good:

```text
function expects 2 arguments, got 3
```

Weak:

```text
semantic analysis failed
```

The message should name the relevant construct, value, symbol, or type. Avoid compiler implementation terms unless the user is interacting with an explicitly low-level command.

## Source labels

The primary label identifies the location the user can act on.

Secondary labels show:

- where a conflicting declaration originated;
- where a block began;
- where an expected type was defined;
- where ownership or visibility comes from;
- where a related pattern or branch appears.

Do not highlight an entire file when a token or expression is available.

## Explanation and action

Use an explanation when the rule is not obvious from the primary message.

Use an action when there is a likely concrete recovery step.

Actions should:

- use current S3 syntax;
- not promise a fix that may change semantics silently;
- prefer a direct edit or command;
- mention migration tooling for removed syntax;
- avoid generic “check your code” advice.

## Accessibility

Diagnostics should reduce the number of reasoning steps required to recover.

- Put the most important fact first.
- Keep sentences short.
- Avoid unnecessary jargon.
- Do not rely on color alone.
- Keep terminology consistent with user docs.
- Show actual and expected types in a stable order.
- Use code formatting for syntax, names, and types.
- Avoid humor or blame.

## Cascades and recovery

After an earlier error produces a recovery value such as an error type:

- suppress derivative operator, call, and assignment errors that add no useful information;
- continue only when later diagnostics are independent and reliable;
- avoid inventing definitions or types that make invalid code appear meaningful;
- keep recovery deterministic for CLI and LSP.

## Removed syntax

A removed syntax diagnostic should state:

- the old form is not accepted;
- the canonical current form;
- whether `ori migrate-syntax` can help;
- any semantic migration that requires manual review.

Removed forms must not fall through to vague parse errors when a dedicated migration diagnostic exists.

## CLI and LSP parity

CLI and LSP may render diagnostics differently, but they must share:

- code;
- severity;
- semantic meaning;
- source ranges;
- core message;
- related information.

The LSP must not maintain a second diagnostic rule set.

## Testing

Every new diagnostic requires:

- a negative fixture;
- exact code assertion;
- important span assertions;
- catalog registration;
- message/action review;
- recovery test when parsing or checking continues;
- LSP evidence when editor behavior is affected.

Golden snapshots are appropriate for full presentation. Focused tests should also assert structured fields so cosmetic rendering changes do not hide semantic drift.

## Warnings

Warnings must identify a real risk or likely mistake. Avoid warnings for style preferences already handled by the formatter.

A warning should define:

- why the code is suspicious;
- false-positive expectations;
- suppression or resolution policy;
- whether it may become an error in a future compatibility cycle.

## Internal failures

Compiler crashes and invariant failures should include:

- phase;
- operation;
- source or project context where safe;
- a stable bug-report route;
- no claim that the user's source is invalid unless a proper diagnostic proves it.

Do not expose memory addresses, secrets, registry credentials, or unnecessary local paths in default output.

## Review checklist

- Is the code registered and semantically unique?
- Does the primary message name the real problem?
- Is the primary span actionable?
- Are actual and expected values clear?
- Is the action valid current Ori?
- Will recovery cause noisy cascades?
- Do CLI and LSP agree?
- Can a reader understand the fix without reading compiler source?
- Is the diagnostic stable enough to document and search?
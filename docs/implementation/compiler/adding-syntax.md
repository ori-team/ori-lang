# Adding or changing syntax

Syntax work is a cross-toolchain change. A parser patch alone is not a complete language feature.

## Before implementation

Confirm:

- the problem cannot be solved with existing syntax;
- the proposed form follows Ori's accessibility principles;
- the grammar is unambiguous;
- precedence and associativity are defined;
- invalid and incomplete forms have diagnostic behavior;
- interaction with formatter, LSP, documentation, and migration is understood;
- compatibility classification is complete;
- an RFC exists when the change is significant or incompatible.

## 1. Normative design

Define:

- lexical form;
- grammar production;
- AST shape;
- semantic meaning;
- type rules;
- control-flow and cleanup behavior;
- evaluation order;
- interaction with generics and traits;
- backend support;
- examples and counterexamples;
- diagnostic codes;
- migration behavior.

The specification should become normative when the design is accepted and implemented. A proposal remains an RFC until then.

## 2. Lexer

Update `ori-lexer` when the feature introduces:

- a keyword;
- an operator or punctuation token;
- a literal form;
- contextual lexical behavior.

Requirements:

- spans cover the exact token;
- trivia behavior remains unchanged unless deliberately specified;
- malformed literals produce lexical diagnostics;
- contextual keywords are preferred when reserving a global keyword would create unnecessary compatibility cost;
- syntax highlighting and keyword export are reviewed.

Add unit tests for valid, invalid, boundary, Unicode, and ambiguous cases as applicable.

## 3. AST

The AST should preserve syntax information needed by:

- semantic analysis;
- formatter;
- diagnostics;
- source mapping;
- migration and documentation tooling.

Do not put backend layouts or runtime symbols into AST nodes.

Prefer explicit variants and typed fields over magic flags. Ensure every new node has a reliable span.

## 4. Parser

Implement the grammar in the focused parser module:

- items in `parse_item`;
- statements in `parse_stmt`;
- expressions in `parse_expr`;
- patterns in `parse_pat`;
- types in `parse_ty`;
- shared token/state behavior in the parser core.

Review:

- precedence and associativity;
- newline sensitivity;
- block termination;
- optional end labels;
- nested poetic-call restrictions;
- parser recovery;
- error-code specificity;
- incomplete input behavior for editors.

A generic `parse.unexpected_token` is acceptable only when no more useful construct-specific diagnostic is available.

## 5. Formatter

The formatter defines canonical presentation.

Add tests for:

- minimal form;
- nesting;
- comments and blank lines;
- long lines;
- surrounding constructs;
- idempotence;
- parse → format → parse semantic equivalence.

Do not accept multiple formatting forms when one canonical form can be emitted consistently.

## 6. Name resolution and signatures

Determine whether the syntax introduces or references:

- definitions;
- bindings;
- scopes;
- imports;
- visibility;
- type parameters;
- associated items;
- labels or control-flow targets.

Assign identities and collect signatures before type checking when later references require them.

## 7. Type checking

Define:

- expected types flowing into the construct;
- inferred types flowing out;
- assignability rules;
- control-flow facts;
- exhaustiveness;
- return behavior;
- cleanup and `using` behavior;
- transferability and concurrency constraints;
- error recovery type.

Record semantic facts needed by lowering. Do not force HIR to reconstruct type-checker decisions from raw AST.

Add positive and negative tests with precise diagnostics.

## 8. HIR lowering

Lower the construct into typed HIR that exposes runtime-relevant behavior explicitly.

Review:

- evaluation order;
- temporary ownership;
- branches and joins;
- cleanup paths;
- async suspension;
- iterator transformation;
- generic substitution;
- debug spans;
- optimization compatibility.

A syntax-only distinction should disappear unless downstream tooling needs it.

## 9. Optimization

Determine whether existing passes:

- preserve the new HIR form;
- can fold or remove it safely;
- require explicit traversal support;
- change debug mappings;
- create a performance cliff.

Add optimized versus unoptimized differential coverage when relevant.

## 10. Native and JIT backends

Implement the typed behavior, not the surface syntax.

Validate:

- value representation;
- control flow;
- runtime calls;
- ownership operations;
- cleanup on all exits;
- ABI and symbol contracts;
- AOT/JIT parity;
- target differences;
- debug information.

Unsupported native constructs must be rejected explicitly with a documented code.

## 11. Runtime

Runtime changes are required only when generated code needs a new native service or representation.

A runtime change requires:

- typed ABI metadata;
- exported symbol;
- safe internal domain logic;
- static and dynamic staging;
- memory/ownership tests;
- Spec 16/19 review;
- security and performance review.

Do not add a runtime primitive for behavior that can remain pure HIR or Ori source without material cost.

## 12. LSP and editor tooling

Review:

- diagnostics;
- incremental parsing/checking;
- completion;
- hover;
- go-to-definition;
- rename;
- symbols;
- semantic tokens;
- inlay hints;
- formatter integration;
- syntax highlighting.

Tooling must consume compiler semantics rather than implement a second parser or type system.

## 13. Migration

For removed or replaced syntax:

- add a dedicated rejection diagnostic;
- update `ori migrate-syntax` for safe mechanical transformations;
- document transformations that require human judgment;
- add old → new migration tests;
- update changelog and compatibility documentation.

Migration should fail visibly when it cannot preserve meaning.

## 14. Conformance package

A complete syntax change normally includes:

- lexer unit tests;
- parser positive and negative tests;
- formatter golden/idempotence tests;
- semantic positive and negative tests;
- AOT and JIT execution;
- explicit rejection of unsupported native shapes;
- multifile cases when scope/imports matter;
- diagnostic catalog entry;
- normative examples;
- user-facing example or guide when the feature is important;
- conformance identifier linking specification and tests.

## 15. Completion review

Before merge, answer:

- Is there one canonical syntax?
- Can the reader infer the important behavior locally?
- Does malformed input produce a useful diagnostic?
- Are all exits and cleanup paths correct?
- Do formatter, LSP, AOT, and JIT agree?
- Is the current specification exact?
- Can users migrate safely?
- Is the implementation in the correct phase?
- Did the change introduce a duplicate source of truth?
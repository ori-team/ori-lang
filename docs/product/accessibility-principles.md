# Accessibility and cognitive-load principles

Accessibility is a product and engineering requirement for Ori. It is not limited to visual presentation and it is not satisfied by describing the language as readable.

These principles apply to language design, diagnostics, documentation, examples, CLI output, editor tooling, and contribution workflows.

## Core principle

A reader should be able to understand important behavior without maintaining a long chain of hidden assumptions.

Ori therefore prefers:

- visible types and failure modes;
- one canonical form for a concept;
- explicit module and ownership boundaries;
- short, concrete diagnostic messages;
- examples that introduce one new idea at a time;
- stable terminology across specification, docs, compiler, and editor tooling.

## Language-design review

A language proposal must consider:

1. **Recognition:** can a reader recognize what the construct does without memorizing unrelated rules?
2. **Locality:** is required information visible near the code that uses it?
3. **Ambiguity:** can the same syntax plausibly mean several things?
4. **Inference depth:** how many unstated steps are needed to understand the resulting type or behavior?
5. **Error recovery:** can a mistake produce a precise, actionable diagnostic?
6. **Visual distinction:** are similar constructs distinguishable without relying only on punctuation density?
7. **Migration cost:** can existing users understand and adopt the change safely?
8. **Tooling:** can formatter, LSP, documentation extraction, and syntax highlighting represent it consistently?

Readability does not mean avoiding advanced capabilities. It means making advanced behavior inspectable.

## Diagnostic requirements

A good diagnostic should answer, in order:

- what happened;
- where it happened;
- why the compiler rejects it, when the reason is not obvious;
- what action is likely to fix it.

Messages should:

- use direct language;
- name the relevant value, type, symbol, or construct;
- avoid jokes, blame, and vague wording;
- avoid requiring the reader to decode compiler internals;
- keep the primary message short and move detail to labels, notes, or actions;
- use stable error codes so users can search and reference them.

See [`../quality/diagnostic-design.md`](../quality/diagnostic-design.md).

## Documentation requirements

User-facing documents should:

- begin with the task or concept the reader is trying to understand;
- use headings that describe content rather than abstract categories;
- keep paragraphs focused;
- explain terminology before relying on it;
- show a minimal valid example before edge cases;
- distinguish normative rules from recommendations;
- include expected output when it matters;
- avoid unexplained abbreviations;
- link to deeper material instead of duplicating it;
- avoid color-only meaning in diagrams or generated sites.

Long documents should include an entry summary and a clear path to the next document.

## Code examples

Examples must:

- compile against the current 0.3.8 implementation unless explicitly archived;
- use canonical S3/0.4 syntax;
- avoid unrelated complexity;
- use descriptive names;
- state assumptions such as files, arguments, or environment variables;
- include a negative example only when it teaches a specific diagnostic or boundary;
- be tested or included in a validation path when presented as runnable.

## CLI and tooling

CLI output should:

- state the failed operation;
- identify the affected path, target, dependency, or symbol;
- separate user action from debug detail;
- preserve stable machine-readable modes where applicable;
- avoid progress noise that hides the final result;
- provide the next command when recovery is deterministic.

Editor tooling should not invent language behavior. Hover, completion, diagnostics, formatter, and navigation must derive from the same semantic contracts as the compiler.

## Review evidence

For significant syntax, diagnostic, documentation, or UI changes, review should include at least one of:

- before/after reading comparison;
- reduced number of steps needed to identify the fix;
- user test or structured review with neurodivergent readers;
- evidence that terminology and examples remain consistent;
- a documented trade-off where explicitness increases verbosity.

## Anti-patterns

Avoid:

- several equivalent syntaxes for the same concept;
- hidden fallback behavior that changes semantics;
- error messages that expose only an internal token or node name;
- examples that require knowledge introduced much later;
- headings such as “Miscellaneous” or “Other” for important rules;
- documentation walls that mix product status, history, architecture, and instructions;
- claiming accessibility without defining verifiable practices.

## Relationship to other contracts

- The manifesto defines the value.
- This document defines product and documentation principles.
- The specification defines accepted language behavior.
- Diagnostic policy defines compiler-message requirements.
- Implementation standards define code-review requirements.
- Quality tests provide evidence that these rules remain true.
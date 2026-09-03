mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::process::Command;

use common::{
    assert_check_output_is_well_formed, assert_diagnostic_spans_within_sources, diagnostic_codes,
    exe_path, normalize_stdout, TestDir,
};
use ori_driver::pipeline::{
    run_check, run_check_source, run_compile, run_doc_with_options, run_parse, DocFormat,
    DocOptions,
};

struct CheckCase {
    name: &'static str,
    files: Vec<(&'static str, &'static str)>,
    expected_codes: &'static [&'static str],
}

#[test]
fn malformed_source_corpus_never_panics_and_keeps_spans_bounded() {
    let mut unbalanced_expression =
        String::from("module app.main\nmain()\n    const value: int = ");
    unbalanced_expression.push_str(&"(".repeat(96));
    unbalanced_expression.push('1');
    unbalanced_expression.push_str(&")".repeat(12));
    unbalanced_expression.push_str("\nend\n");

    let mut deep_unary = String::from("module app.main\nmain()\n    const value: bool = ");
    deep_unary.push_str(&"not ".repeat(512));
    deep_unary.push_str("true\nend\n");

    let mut deep_pattern = String::from("module app.main\nmain()\n    match true\n        case ");
    deep_pattern.push_str(&"some(".repeat(512));
    deep_pattern.push_str("true");
    deep_pattern.push_str(&")".repeat(512));
    deep_pattern.push_str(":\n            return\n    end\nend\n");

    let cases = [
        ("empty_file", ""),
        ("namespace_without_path", "namespace\n"),
        (
            "unterminated_block_comment",
            "module app.main\n--| comment without terminator\nmain()\nend\n",
        ),
        (
            "unterminated_string",
            "module app.main\nmain()\n    const text: string = \"open\nend\n",
        ),
        (
            "unicode_and_bad_tokens",
            "module app.main\nmain()\n    const cafe: string = \"cafe emoji \u{1F642}\"\n    @@@\nend\n",
        ),
        (
            "bad_import_shape",
            "module app.main\nimport app.\nmain()\nend\n",
        ),
        (
            "invalid_arrow_token",
            "module app.main\nmain()\n    using value -->\nend\n",
        ),
        (
            "short_result_type",
            "module app.main\nmain()\n    const value: result[int] = err(1)\nend\n",
        ),
        (
            "short_map_type",
            "module app.main\nmain()\n    const value: map[string] = none\nend\n",
        ),
        ("unbalanced_expression", unbalanced_expression.as_str()),
        ("deep_unary_expression", deep_unary.as_str()),
        ("deep_constructor_pattern", deep_pattern.as_str()),
    ];

    for (name, source) in cases {
        let dir = TestDir::new(name);
        dir.write("main.orl", source);
        let parsed = catch_unwind(AssertUnwindSafe(|| run_parse(&dir.path("main.orl"))))
            .unwrap_or_else(|panic| panic!("parser panicked for case `{name}`: {panic:?}"))
            .unwrap_or_else(|error| {
                panic!("parser returned infrastructure error for `{name}`: {error}")
            });

        assert_diagnostic_spans_within_sources(&parsed.cache, &parsed.diagnostics);
        assert!(
            parsed.diagnostics.len() <= 64,
            "case `{name}` emitted too many diagnostics: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn recursive_parser_constructor_matrix_has_cli_lsp_parity() {
    const DEPTH: usize = 256;
    let expression_source = |expression: String| {
        format!("module app.main\nmain()\n    const value = {expression}\nend\n")
    };
    let pattern_source = |pattern: String| {
        format!(
            "module app.main\nmain()\n    match true\n    case {pattern}:\n        return\n    end\nend\n"
        )
    };
    let nested = |prefix: &str, value: &str, suffix: &str| {
        format!("{}{value}{}", prefix.repeat(DEPTH), suffix.repeat(DEPTH))
    };

    let cases = vec![
        // Keep adjacent `-` tokens separated: `--` starts an Ori comment.
        ("unary_neg", expression_source(nested("- ", "1", ""))),
        ("unary_not", expression_source(nested("not ", "true", ""))),
        ("unary_bit_not", expression_source(nested("~", "1", ""))),
        ("unary_try", expression_source(nested("try ", "value", ""))),
        (
            "unary_await",
            expression_source(nested("await ", "value", "")),
        ),
        ("grouped", expression_source(nested("(", "1", ")"))),
        ("call", expression_source(nested("id(", "1", ")"))),
        (
            "tuple_expression",
            expression_source(nested("tuple(", "1", ", 0)")),
        ),
        ("list", expression_source(nested("[", "1", "]"))),
        ("set", expression_source(nested("set {", "1", "}"))),
        (
            "struct_literal",
            expression_source(nested("Node { value: ", "1", " }")),
        ),
        (
            "if_expression",
            expression_source(nested("if true then ", "1", " else 0")),
        ),
        (
            "match_expression",
            expression_source(nested("match true case true: ", "1", " case else: 0 end")),
        ),
        (
            "closure_expression",
            expression_source(nested("() => ", "1", "")),
        ),
        ("pattern_some", pattern_source(nested("some(", "true", ")"))),
        ("pattern_ok", pattern_source(nested("ok(", "true", ")"))),
        ("pattern_err", pattern_source(nested("err(", "true", ")"))),
        (
            "pattern_tuple",
            pattern_source(nested("tuple(", "true", ", false)")),
        ),
        (
            "pattern_variant",
            pattern_source(nested("Node(value: ", "true", ")")),
        ),
    ];

    for (name, source) in cases {
        let dir = TestDir::new(name);
        let path = dir.path("main.orl");
        dir.write("main.orl", &source);

        let cli = catch_unwind(AssertUnwindSafe(|| run_check(&path)))
            .unwrap_or_else(|panic| panic!("CLI check panicked for `{name}`: {panic:?}"))
            .unwrap();
        let lsp = catch_unwind(AssertUnwindSafe(|| run_check_source(&path, source.clone())))
            .unwrap_or_else(|panic| panic!("LSP check panicked for `{name}`: {panic:?}"))
            .unwrap();

        for (path_name, output) in [("CLI", &cli), ("LSP", &lsp)] {
            assert_diagnostic_spans_within_sources(&output.cache, &output.diagnostics);
            let nesting_count = output
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "parse.nesting_too_deep")
                .count();
            assert_eq!(
                nesting_count, 1,
                "{path_name} must emit one bounded nesting diagnostic for `{name}`: {:?}",
                output.diagnostics
            );
        }
    }
}

#[test]
fn fixed_arity_type_matrix_never_panics() {
    let constructors = [
        "optional", "list", "set", "range", "lazy", "handle", "result", "map",
    ];
    let argument_lists = [
        "[]",
        "[int]",
        "[int, string]",
        "[int, string, bool]",
        "<>",
        "<int>",
        "<int, string>",
        "<int, string, bool>",
    ];

    for constructor in constructors {
        for arguments in argument_lists {
            let source = format!(
                "module app.main\nmain()\n    const value: {constructor}{arguments} = none\nend\n"
            );
            let dir = TestDir::new("fixed_arity_type_matrix");
            dir.write("main.orl", &source);
            let parsed = catch_unwind(AssertUnwindSafe(|| run_parse(&dir.path("main.orl"))))
                .unwrap_or_else(|panic| {
                    panic!(
                        "parser panicked for type form `{constructor}{arguments}`: {panic:?}"
                    )
                })
                .unwrap_or_else(|error| {
                    panic!(
                        "parser returned infrastructure error for type form `{constructor}{arguments}`: {error}"
                    )
                });
            assert_diagnostic_spans_within_sources(&parsed.cache, &parsed.diagnostics);
        }
    }
}

#[test]
fn deterministic_token_mutation_corpus_never_panics() {
    let seeds = [
        "module app.main\nmain()\n    const x: int = 1\nend\n",
        "module app.main\nstruct User\n    name: string\nend\nmain()\nend\n",
        "module app.main\nmain()\n    match true\n        case true:\n            return\n    end\nend\n",
    ];
    let fragments = [
        "",
        "--|",
        "\"",
        "end\nend\nend",
        "case else:",
        "if some(x) =",
        "try",
        "await",
        "\u{1F642}",
        "0xZZ",
        "import app..bad",
    ];

    for index in 0..96 {
        let seed = seeds[index % seeds.len()];
        let fragment = fragments[index % fragments.len()];
        let insert_at = seed
            .char_indices()
            .nth((index * 7) % seed.chars().count())
            .map(|(byte, _)| byte)
            .unwrap_or(seed.len());
        let source = format!("{} {} {}", &seed[..insert_at], fragment, &seed[insert_at..]);

        let dir = TestDir::new("token_mutation");
        dir.write("main.orl", &source);
        let parsed = catch_unwind(AssertUnwindSafe(|| run_parse(&dir.path("main.orl"))))
            .unwrap_or_else(|panic| {
                panic!("parser panicked for mutation {index}: {panic:?}\n{source}")
            })
            .unwrap_or_else(|error| {
                panic!("parser returned infrastructure error for mutation {index}: {error}")
            });

        assert_diagnostic_spans_within_sources(&parsed.cache, &parsed.diagnostics);
    }
}

#[test]
fn semantic_security_rules_report_stable_diagnostic_codes() {
    let cases = vec![
        CheckCase {
            name: "undefined_name",
            files: vec![(
                "main.orl",
                "module app.main\nmain()\n    const value: int = missing\nend\n",
            )],
            expected_codes: &["name.undefined"],
        },
        CheckCase {
            name: "const_reassignment",
            files: vec![(
                "main.orl",
                "module app.main\nmain()\n    const value: int = 1\n    value = 2\nend\n",
            )],
            expected_codes: &["bind.const_reassignment"],
        },
        CheckCase {
            name: "check_message_must_be_string",
            files: vec![(
                "main.orl",
                "module app.main\nmain()\n    check true, 42\n    const after: int = 1\nend\n",
            )],
            expected_codes: &["parse.check_message_literal"],
        },
        CheckCase {
            name: "dynamic_check_message_must_be_string",
            files: vec![(
                "main.orl",
                "module app.main\nmain()\n    const message: string = \"failure\"\n    check true, message\n    const after: int = 1\nend\n",
            )],
            expected_codes: &["parse.check_message_literal"],
        },
        CheckCase {
            name: "duplicate_struct_field",
            files: vec![(
                "main.orl",
                "module app.main\nstruct User\n    name: string\n    name: string\nend\nmain()\nend\n",
            )],
            expected_codes: &["bind.duplicate_field"],
        },
        CheckCase {
            name: "non_exhaustive_match",
            files: vec![(
                "main.orl",
                r#"module app.main
enum Color
    Red
    Blue
end
label(color: Color) -> string
    match color
        case Red:
            return "red"
    end
end
main()
end
"#,
            )],
            expected_codes: &["match.non_exhaustive"],
        },
        CheckCase {
            name: "await_outside_async",
            files: vec![(
                "main.orl",
                "module app.main\nimport ori.task = task\nmain()\n    await task.sleep(1)\nend\n",
            )],
            expected_codes: &["async.await_outside_async"],
        },
        CheckCase {
            name: "non_transferable_spawn_capture",
            files: vec![(
                "main.orl",
                r#"module app.main
import ori.task = task
import ori.lazy = lz
main()
    const delayed: lazy[int] = lz.once(() => 1)
    const callback: func() -> int = () => lz.force(delayed)
    const job: task.Job[int] = task.spawn(() => callback())
end
"#,
            )],
            expected_codes: &["async.capture_not_transferable"],
        },
        CheckCase {
            name: "unknown_stdlib_module",
            files: vec![(
                "main.orl",
                "module app.main\nimport ori.this_module_does_not_exist\nmain()\nend\n",
            )],
            expected_codes: &["bind.stdlib_module_unknown"],
        },
        CheckCase {
            name: "generic_constraint",
            files: vec![(
                "main.orl",
                r#"module app.main
trait Named
    name(self) -> string
end
read_name for T: Named (value: T) -> string
    return value.name()
end
main()
    const text: string = read_name(1)
end
"#,
            )],
            expected_codes: &["generic.constraint_not_satisfied"],
        },
        CheckCase {
            name: "private_cross_module_access",
            files: vec![
                (
                    "util.orl",
                    "module app.util\nsecret() -> int\n    return 42\nend\n",
                ),
                (
                    "main.orl",
                    "module app.main\nimport app.util = util\nmain()\n    const value: int = util.secret()\nend\n",
                ),
            ],
            expected_codes: &["name.private"],
        },
        CheckCase {
            name: "selective_import_unknown_member",
            files: vec![
                (
                    "app/math.orl",
                    "module app.math\npublic add(a: int, b: int) -> int\n    return a + b\nend\n",
                ),
                (
                    "main.orl",
                    "module app.main\nimport app.math (missing)\nmain()\nend\n",
                ),
            ],
            expected_codes: &["bind.import_member_unknown"],
        },
        CheckCase {
            name: "namespace_file_mismatch",
            files: vec![
                (
                    "util.orl",
                    "module app.other\npublic value() -> int\n    return 1\nend\n",
                ),
                (
                    "main.orl",
                    "module app.main\nimport app.util = util\nmain()\n    const value: int = util.value()\nend\n",
                ),
            ],
            expected_codes: &["project.namespace_file_mismatch"],
        },
        CheckCase {
            name: "circular_import",
            files: vec![
                (
                    "a.orl",
                    "module app.a\nimport app.b\nmain()\nend\n",
                ),
                (
                    "b.orl",
                    "module app.b\nimport app.a\n",
                ),
            ],
            expected_codes: &["project.circular_import"],
        },
    ];

    for case in cases {
        let dir = TestDir::new(case.name);
        for (name, source) in case.files {
            dir.write(name, source);
        }
        let entry = if case.name == "circular_import" {
            dir.path("a.orl")
        } else {
            dir.path("main.orl")
        };
        let out = catch_unwind(AssertUnwindSafe(|| run_check(&entry)))
            .unwrap_or_else(|panic| panic!("checker panicked for `{}`: {panic:?}", case.name))
            .unwrap_or_else(|error| {
                panic!("checker infrastructure error for `{}`: {error}", case.name)
            });

        assert!(out.has_errors, "case `{}` unexpectedly passed", case.name);
        assert_check_output_is_well_formed(&out);
        let codes = diagnostic_codes(&out);
        for expected in case.expected_codes {
            assert!(
                codes.contains(expected),
                "case `{}` expected `{expected}`, got {codes:?}\n{:?}",
                case.name,
                out.diagnostics
            );
        }
    }
}

#[test]
fn generated_documentation_html_escapes_doc_comment_content() {
    let dir = TestDir::new("doc_html_escaping");
    dir.write(
        "main.orl",
        r#"module app.main

--|
<script>alert("x")</script>

@param name <img src=x onerror=alert(1)>
@returns <b>safe text only</b>
|--
public greet(name: string) -> string
    return name
end

main()
end
"#,
    );

    let out = run_doc_with_options(
        &dir.path("main.orl"),
        DocOptions {
            format: DocFormat::Html,
        },
    )
    .unwrap();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.html.contains("&lt;script&gt;"));
    assert!(out.html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(out.html.contains("&lt;b&gt;safe text only&lt;/b&gt;"));
    assert!(!out.html.contains("<script>"));
    assert!(!out.html.contains("<img"));
    assert!(!out.html.contains("<b>safe text only</b>"));
}

#[test]
fn native_runtime_composite_program_runs_under_leak_check() {
    let dir = TestDir::new("native_runtime_security_leak_check");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.test = test

struct Buffer
    items: list[int]
end

exercise() -> int
    const values: list[int] = lists.new()
    lists.push(values, 10)
    lists.push(values, 20)
    const buffer: Buffer = Buffer {items: values}
    return lists.len(buffer.items)
end

main()
    const size: int = exercise()
    const leaked: int = test.assert_no_leaks("native_runtime_composite")
    io.print("size:" + string(size))
    io.print("leaks:" + string(leaked))
end
"#,
    );

    let exe = exe_path(&dir, "native_runtime_security_leak_check");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .env("ORI_TEST_LEAK_CHECK", "1")
        .output()
        .unwrap();
    let stdout = normalize_stdout(output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "program failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "size:2\nleaks:0");
}

#[test]
fn hostile_inputs_and_pathological_literals_never_panic_in_checker() {
    let cases = [
        (
            "pathological_identifier",
            format!("module app.main\nmain()\n    const {}: int = 1\nend\n", "a".repeat(4000)),
        ),
        (
            "pathological_integer_literal",
            format!("module app.main\nmain()\n    const x: int = {}\nend\n", "9".repeat(4000)),
        ),
        (
            "pathological_hex_literal",
            format!("module app.main\nmain()\n    const x: int = 0x{}\nend\n", "F".repeat(4000)),
        ),
        (
            "pathological_float_exponent",
            "module app.main\nmain()\n    const x: float = 1.0e999999999999999999999\nend\n".to_string(),
        ),
        (
            "unclosed_fstring_interpolation",
            "module app.main\nmain()\n    const s: string = f\"unclosed {1 + 2\nend\n".to_string(),
        ),
        (
            "malformed_cfg_predicate",
            "module app.main\n@cfg(all(any(not(), target = \"unknown\"), feature = 123))\nmain()\nend\n".to_string(),
        ),
        (
            "embedded_null_byte_in_string",
            "module app.main\nmain()\n    const s: string = \"embedded \0 null\"\nend\n".to_string(),
        ),
    ];

    for (name, source) in cases {
        let dir = TestDir::new(name);
        dir.write("main.orl", &source);
        let checked = catch_unwind(AssertUnwindSafe(|| run_check(&dir.path("main.orl"))))
            .unwrap_or_else(|panic| panic!("checker panicked for hostile case `{name}`: {panic:?}"))
            .unwrap_or_else(|error| {
                panic!("checker returned infrastructure error for `{name}`: {error}")
            });
        assert_diagnostic_spans_within_sources(&checked.cache, &checked.diagnostics);
    }
}

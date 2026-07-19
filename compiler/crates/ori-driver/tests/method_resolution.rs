use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::{run_build, run_check, CheckOutput};

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_method_test_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.path(name), source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn diagnostic_codes(out: &CheckOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn build_lowers_inherent_method_call() {
    let dir = TestDir::new("inherent_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
    score: int

    add(self, bonus: int) -> int
        return self.score + bonus
    end
end

main()
    const player: Player = Player {score: 7}
    const total: int = player.add(5)
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build
        .c_source
        .contains("ORI__app_dot_main_dot_Player_dot_add"));
    assert!(
        build
            .c_source
            .contains("ORI__app_dot_main_dot_Player_dot_add(player, INT64_C(5))"),
        "{}",
        build.c_source
    );
}

#[test]
fn check_reports_inherent_method_argument_type_mismatch() {
    let dir = TestDir::new("inherent_method_arg_type");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
    score: int

    add(self, bonus: int) -> int
        return self.score + bonus
    end
end

main()
    const player: Player = Player {score: 7}
    const total: int = player.add("bad")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn build_lowers_apply_method_call() {
    let dir = TestDir::new("apply_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
    score: int
end

trait Entity
    id(self) -> int
end

apply Player use Entity
    id(self) -> int
        return self.score
    end
end

main()
    const player: Player = Player {score: 42}
    const id: int = player.id()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build
        .c_source
        .contains("ORI__app_dot_main_dot_Player_dot_Entity_dot_id"));
    assert!(
        build
            .c_source
            .contains("ORI__app_dot_main_dot_Player_dot_Entity_dot_id(player)"),
        "{}",
        build.c_source
    );
}

#[test]
fn check_reports_ambiguous_trait_method_call() {
    let dir = TestDir::new("ambiguous_trait_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Thing
    name: string
end

trait Alpha
    output(self) -> string
end

trait Beta
    output(self) -> string
end

apply Thing use Alpha
    output(self) -> string
        return "alpha"
    end
end

apply Thing use Beta
    output(self) -> string
        return "beta"
    end
end

main()
    const thing: Thing = Thing {name: "x"}
    const text: string = thing.output()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(diagnostic_codes(&out).contains(&"type.ambiguous_method"));
}

#[test]
fn build_lowers_qualified_trait_method_call() {
    let dir = TestDir::new("qualified_trait_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Thing
    name: string
end

trait Alpha
    output(self) -> string
end

trait Beta
    output(self) -> string
end

apply Thing use Alpha
    output(self) -> string
        return "alpha"
    end
end

apply Thing use Beta
    output(self) -> string
        return "beta"
    end
end

main()
    const thing: Thing = Thing {name: "x"}
    const alpha: string = Alpha.output(thing)
    const beta: string = Beta.output(thing)
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build
        .c_source
        .contains("ORI__app_dot_main_dot_Thing_dot_Alpha_dot_output"));
    assert!(build
        .c_source
        .contains("ORI__app_dot_main_dot_Thing_dot_Beta_dot_output"));
}

#[test]
fn build_lowers_default_trait_method_call() {
    let dir = TestDir::new("default_trait_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
    score: int
end

trait Entity
    id(self) -> int
        return 7
    end
end

apply Player use Entity
end

main()
    const player: Player = Player {score: 42}
    const id: int = player.id()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build
        .c_source
        .contains("ORI__app_dot_main_dot_Entity_dot_id"));
}

#[test]
fn check_reports_missing_required_trait_method() {
    let dir = TestDir::new("missing_trait_method");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
end

trait Entity
    id(self) -> int
    tick(self) -> void
end

apply Player use Entity
    id(self) -> int
        return 1
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"impl.missing_method"));
}

#[test]
fn check_reports_trait_method_signature_mismatch() {
    let dir = TestDir::new("trait_signature_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
end

trait Cloneable
    merge(self, other: Self) -> Self
end

apply Player use Cloneable
    merge(self, other: int) -> Player
        return self
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"impl.wrong_signature"));
}

#[test]
fn check_reports_trait_method_mut_mismatch() {
    let dir = TestDir::new("trait_mut_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

struct Bag
end

trait Stackable
    mut push(self) -> void
end

apply Bag use Stackable
    push(self) -> void
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"impl.mut_mismatch"));
}

#[test]
fn check_reports_implicit_self_mut_method_on_const_receiver() {
    let dir = TestDir::new("implicit_self_const_receiver");
    dir.write(
        "main.orl",
        r#"module app.main

struct Counter
    value: int

    mut increment()
        self.value = self.value + 1
    end
end

main()
    const counter: Counter = Counter {value: 1}
    counter.increment()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"mut.const_method_call"));
}

#[test]
fn build_lowers_free_bind_as_inherent_method() {
    let dir = TestDir::new("free_bind_inherent");
    dir.write(
        "main.orl",
        r#"module app.main

struct Point
    x: int
    y: int
end

pointDebugName(p: Point) -> string
    return "point"
end

apply Point
    debugName = pointDebugName
end

main()
    const p: Point = Point { x: 1, y: 2 }
    const name: string = p.debugName()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(
        build
            .c_source
            .contains("ORI__app_dot_main_dot_pointDebugName")
            || build.c_source.contains("pointDebugName"),
        "{}",
        build.c_source
    );
}

#[test]
fn build_lowers_free_method_only_on_apply() {
    let dir = TestDir::new("free_method_only");
    dir.write(
        "main.orl",
        r#"module app.main

struct Point
    x: int
end

apply Point
    freeMethod(self) -> int
        return self.x
    end
end

main()
    const p: Point = Point { x: 3 }
    const v: int = p.freeMethod()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(
        build
            .c_source
            .contains("ORI__app_dot_main_dot_Point_dot_freeMethod"),
        "{}",
        build.c_source
    );
}

#[test]
fn check_rejects_free_member_after_use_section() {
    let dir = TestDir::new("free_after_use");
    dir.write(
        "main.orl",
        r#"module app.main

struct Point
end

trait Marker
end

apply Point
    use Marker
    end

    freeLate(self) -> int
        return 1
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(diagnostic_codes(&out).contains(&"parse.apply_member_after_use"));
}

#[test]
fn build_lowers_apply_bind_to_free_function() {
    let dir = TestDir::new("apply_bind");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
    score: int
end

trait Entity
    id(self) -> int
end

playerId(player: Player) -> int
    return player.score
end

apply Player use Entity
    id = playerId
end

main()
    const player: Player = Player { score: 99 }
    const id: int = player.id()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    // Bind reuses the free function path (no Type.Trait.method wrapper).
    assert!(
        build.c_source.contains("ORI__app_dot_main_dot_playerId")
            || build.c_source.contains("playerId"),
        "{}",
        build.c_source
    );
}

#[test]
fn check_rejects_legacy_implement_and_apply_trait_to() {
    let dir = TestDir::new("legacy_apply_forms");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
end

trait Entity
    id(self) -> int
end

implement Entity for Player
    id(self) -> int
        return 1
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.implement_removed"));

    dir.write(
        "legacy_to.orl",
        r#"module app.main

struct Player
end

trait Entity
    id(self) -> int
end

apply Entity to Player
    id(self) -> int
        return 1
    end
end
"#,
    );
    let out = run_check(&dir.path("legacy_to.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.apply_trait_to_removed"));

    dir.write(
        "legacy_for.orl",
        r#"module app.main

struct Player
end

trait Entity
    id(self) -> int
end

apply Entity for Player
    id(self) -> int
        return 1
    end
end
"#,
    );
    let out = run_check(&dir.path("legacy_for.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.apply_trait_to_removed"));
}

#[test]
fn check_reports_duplicate_apply_pair() {
    let dir = TestDir::new("duplicate_apply");
    dir.write(
        "main.orl",
        r#"module app.main

struct Player
end

trait Entity
    id(self) -> int
end

apply Player use Entity
    id(self) -> int
        return 1
    end
end

apply Player use Entity
    id(self) -> int
        return 2
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.duplicate_implement"));
}

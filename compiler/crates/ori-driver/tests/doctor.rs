//! Regression: `ori doctor` reports stdlib and runtime health.

use ori_driver::pipeline::{run_doctor, DoctorStatus};

#[test]
fn doctor_reports_stdlib_root_in_dev_layout() {
    let report = run_doctor();
    assert!(
        report.checks.iter().any(|c| c.name == "stdlib root"),
        "doctor should include stdlib root check"
    );
    let stdlib = report
        .checks
        .iter()
        .find(|c| c.name == "stdlib root")
        .expect("stdlib check");
    assert_eq!(
        stdlib.status,
        DoctorStatus::Ok,
        "dev layout should find stdlib/: {}",
        stdlib.detail
    );
}

#[test]
fn doctor_includes_linker_and_run_mode() {
    let report = run_doctor();
    assert!(report.checks.iter().any(|c| c.name == "linker strategy"));
    assert!(report.checks.iter().any(|c| c.name == "ori run mode"));
}

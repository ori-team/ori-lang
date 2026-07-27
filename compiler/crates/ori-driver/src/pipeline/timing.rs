//! Optional diagnostics for measuring the driver pipeline.

/// Emit an internal timing sample when explicitly requested by the developer.
///
/// This hook is deliberately side-effect free unless
/// `ORI_INTERNAL_PIPELINE_TIMINGS` is set, so normal compiler output and public
/// diagnostics remain unchanged.
pub(super) fn report_internal_pipeline_timing(stage: &str, elapsed: std::time::Duration) {
    if std::env::var_os("ORI_INTERNAL_PIPELINE_TIMINGS").is_some() {
        eprintln!(
            "ORI_INTERNAL_PIPELINE_TIMINGS {stage}: {}ms",
            elapsed.as_millis()
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::health::{check_system_health, is_semver_greater, HealthStatus};

    #[test]
    fn test_system_health_report_structure() {
        let report = check_system_health();
        assert!(!report.items.is_empty(), "Health report items must not be empty");
        assert!(!report.kernel_version.is_empty(), "Kernel version must be detected");
        assert!(
            report.overall_status == HealthStatus::Passed
                || report.overall_status == HealthStatus::Warning
                || report.overall_status == HealthStatus::Failed
        );
    }

    #[test]
    fn test_semver_comparison_logic() {
        assert!(is_semver_greater("2.0.2", "2.0.3"));
        assert!(is_semver_greater("2.0.2", "2.1.0"));
        assert!(is_semver_greater("2.0.2", "3.0.0"));
        assert!(is_semver_greater("v2.0.2", "v2.0.3"));
        assert!(!is_semver_greater("2.0.2", "2.0.2"));
        assert!(!is_semver_greater("2.0.3", "2.0.2"));
        assert!(!is_semver_greater("2.1.0", "2.0.9"));
    }
}

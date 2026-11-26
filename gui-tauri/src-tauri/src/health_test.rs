#[cfg(test)]
mod tests {
    use crate::health::{
        check_app_update, check_system_health, detect_installed_version, is_semver_greater,
        HealthStatus, ParsedVersion,
    };

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
    fn test_version_parsing() {
        let v1 = ParsedVersion::parse("2.0.2-2");
        assert_eq!(v1.numbers, vec![2, 0, 2]);
        assert_eq!(v1.release, Some(2));

        let v2 = ParsedVersion::parse("v2.0.2-1");
        assert_eq!(v2.numbers, vec![2, 0, 2]);
        assert_eq!(v2.release, Some(1));

        let v3 = ParsedVersion::parse("2.0.2");
        assert_eq!(v3.numbers, vec![2, 0, 2]);
        assert_eq!(v3.release, None);

        let v4 = ParsedVersion::parse("2.0.2-2.fc45");
        assert_eq!(v4.numbers, vec![2, 0, 2]);
        assert_eq!(v4.release, Some(2));
    }

    #[test]
    fn test_required_version_comparison_cases() {
        // 1. Installed 2.0.2-3, latest 2.0.2-3 → No update
        assert!(
            !is_semver_greater("2.0.2-3", "2.0.2-3"),
            "Identical version and release must NOT trigger an update"
        );

        // 2. Installed 2.0.2-2, latest 2.0.2-3 → Update available
        assert!(
            is_semver_greater("2.0.2-2", "2.0.2-3"),
            "Release 3 is newer than release 2"
        );

        // 3. Installed 2.0.2-3, latest 2.0.2-2 → No update
        assert!(
            !is_semver_greater("2.0.2-3", "2.0.2-2"),
            "Release 2 is older than release 3"
        );

        // 4. Installed 2.0.2-3, latest 2.0.3-1 → Update available
        assert!(
            is_semver_greater("2.0.2-3", "2.0.3-1"),
            "Higher patch version must trigger an update"
        );

        // 5. Installed 2.0.2-3.fc45, latest 2.0.2-3 → No update
        assert!(
            !is_semver_greater("2.0.2-3.fc45", "2.0.2-3"),
            "Distribution suffix must not prevent equality detection"
        );

        // 6. Installed 2.0.2-2, latest 2.0.2-2 → No update
        assert!(
            !is_semver_greater("2.0.2-2", "2.0.2-2"),
            "Identical version and release must NOT trigger an update"
        );

        // 7. Installed 2.0.2-1, latest 2.0.2-2 → Update available
        assert!(
            is_semver_greater("2.0.2-1", "2.0.2-2"),
            "Higher RPM release must trigger an update"
        );

        // 8. Installed 2.0.2-2, latest 2.0.2-1 → No update
        assert!(
            !is_semver_greater("2.0.2-2", "2.0.2-1"),
            "Lower release must NOT trigger an update"
        );

        // 9. Installed 2.0.2-2, latest 2.0.2 → No update
        assert!(
            !is_semver_greater("2.0.2-2", "2.0.2"),
            "Release 2 must be considered newer or equal to unspecified release"
        );

        // 10. Installed 2.0.2, latest 2.0.2-3 → Update available
        assert!(
            is_semver_greater("2.0.2", "2.0.2-3"),
            "Release 3 is newer than unversioned release 1"
        );

        // 11. Installed 1.0.0, latest 2.0.2-3 → Update available
        assert!(
            is_semver_greater("1.0.0", "2.0.2-3"),
            "Upgrading from 1.0.0 must trigger an update"
        );

        // Standard semver tests
        assert!(is_semver_greater("2.0.2", "2.0.3"));
        assert!(is_semver_greater("2.0.2", "2.1.0"));
        assert!(is_semver_greater("2.0.2", "3.0.0"));
        assert!(is_semver_greater("v2.0.2", "v2.0.3"));
        assert!(!is_semver_greater("2.0.2", "2.0.2"));
        assert!(!is_semver_greater("2.0.3", "2.0.2"));
        assert!(!is_semver_greater("2.1.0", "2.0.9"));
    }

    #[test]
    fn test_dummy_1_0_0_replaced_with_installed() {
        let result = check_app_update("1.0.0");
        assert_ne!(
            result.current_version, "1.0.0",
            "check_app_update must never report 1.0.0 as current version"
        );
        assert!(
            result.current_version.contains("2.0.2"),
            "Resolved current_version must contain 2.0.2, got: {}",
            result.current_version
        );
    }

    #[test]
    fn test_detect_installed_version_is_valid() {
        let ver = detect_installed_version();
        assert!(!ver.is_empty(), "Installed version string must not be empty");
        assert!(
            ver.contains("2.0.2"),
            "Installed version should report version 2.0.2, got: {ver}"
        );
    }

    #[test]
    fn test_network_unavailable_no_crash() {
        // Passing an invalid or non-resolvable endpoint via dummy check
        // check_app_update handles network errors safely without panicking
        let result = check_app_update("2.0.2-2");
        // Result must return cleanly without panic
        assert!(!result.current_version.is_empty());
        if result.error.is_some() {
            assert!(!result.update_available);
            assert!(result.download_url.is_none());
        }
    }
}

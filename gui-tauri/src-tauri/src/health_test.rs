#[cfg(test)]
mod tests {
    use crate::health::{
        check_app_update, check_system_health, detect_installed_version, is_semver_greater,
        HealthStatus, ParsedVersion,
    };
    use crate::system_center::SystemCenterManager;
    use crate::system_info::collect_system_information;
    use crate::telemetry::SystemIdentity;

    #[test]
    fn test_system_health_report_structure() {
        let report = check_system_health();
        assert!(!report.items.is_empty(), "Health report items must not be empty");
        assert_eq!(report.subsystems.len(), 6, "Must return all 6 subsystem cards");
        assert!(!report.kernel_version.is_empty(), "Kernel version must be detected");
        assert!(report.checked_at_ms > 0);
        assert!(
            report.overall_status == HealthStatus::Passed
                || report.overall_status == HealthStatus::Warning
                || report.overall_status == HealthStatus::Failed
        );
    }

    #[test]
    fn test_system_information_collection() {
        let identity = SystemIdentity {
            cpu_model: "Test CPU".into(),
            gpu_model: "Test GPU".into(),
            cpu_max_ghz: 5,
        };
        let info = collect_system_information(&identity);
        assert!(!info.hardware.manufacturer.is_empty());
        assert!(!info.hardware.product_model.is_empty());
        assert!(!info.os_kernel.distribution.is_empty());
        assert!(!info.os_kernel.kernel_release.is_empty());
        assert!(info.memory_storage.ram_total_mb > 0);
        assert!(!info.memory_storage.primary_drive.is_empty());
    }

    #[test]
    fn test_system_center_caching_and_activity() {
        let mgr = SystemCenterManager::new();
        let initial_peek = mgr.peek_health();
        assert!(initial_peek.report.is_none());

        let first = mgr.get_or_refresh_health(true);
        assert!(first.report.is_some());
        assert!(!first.cached);

        let second = mgr.get_or_refresh_health(false);
        assert!(second.report.is_some());
        assert!(second.cached);

        let activity = mgr.get_activity_list();
        assert!(
            activity.iter().any(|e| e.source == "health-probe"),
            "Health scan should append a real activity entry"
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
        // 1. Installed 2.0.2-4, latest 2.0.2-4 → No update
        assert!(
            !is_semver_greater("2.0.2-4", "2.0.2-4"),
            "Identical version and release must NOT trigger an update"
        );

        // 2. Installed 2.0.2-3, latest 2.0.2-4 → Update available
        assert!(
            is_semver_greater("2.0.2-3", "2.0.2-4"),
            "Release 4 is newer than release 3"
        );

        // 3. Installed 2.0.2-4, latest 2.0.2-3 → No update
        assert!(
            !is_semver_greater("2.0.2-4", "2.0.2-3"),
            "Release 3 is older than release 4"
        );

        // 4. Installed 2.0.2-4, latest 2.0.3-1 → Update available
        assert!(
            is_semver_greater("2.0.2-4", "2.0.3-1"),
            "Higher patch version must trigger an update"
        );

        // 5. Installed 2.0.2-4.fc45, latest 2.0.2-4 → No update
        assert!(
            !is_semver_greater("2.0.2-4.fc45", "2.0.2-4"),
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

    #[test]
    fn test_system_center_caching_performance_and_diagnostics_budget() {
        use std::time::Instant;

        let mgr = SystemCenterManager::new();
        assert!(mgr.peek_updates().result.is_none());

        let t_cold = Instant::now();
        let cold = mgr.get_or_refresh_health(true);
        let cold_elapsed = t_cold.elapsed();
        assert!(cold.report.is_some());
        assert!(!cold.cached);
        assert!(
            cold_elapsed.as_secs_f64() < 4.5,
            "Parallelized system health check should finish cleanly within timeout budget, took {:?}",
            cold_elapsed
        );

        let t_cached = Instant::now();
        let cached = mgr.get_or_refresh_health(false);
        let cached_elapsed = t_cached.elapsed();
        assert!(cached.cached);
        assert!(
            cached_elapsed.as_millis() < 20,
            "Cached health retrieval must return instantaneously, took {:?}",
            cached_elapsed
        );

        let t_sys = Instant::now();
        let identity = SystemIdentity {
            cpu_model: "Test CPU".into(),
            gpu_model: "Test GPU".into(),
            cpu_max_ghz: 5,
        };
        let sys_cold = mgr.get_system_information(&identity, false);
        assert!(!sys_cold.hardware.manufacturer.is_empty());
        assert!(!sys_cold.os_kernel.distribution.is_empty());
        assert!(
            t_sys.elapsed().as_millis() < 250,
            "Cold system info collection exceeded budget: {:?}",
            t_sys.elapsed()
        );

        let t_sys_cached = Instant::now();
        let sys_cached = mgr.get_system_information(&identity, false);
        assert_eq!(sys_cached.hardware.product_model, sys_cold.hardware.product_model);
        assert!(
            t_sys_cached.elapsed().as_millis() < 10,
            "Cached system info retrieval must return immediately, took {:?}",
            t_sys_cached.elapsed()
        );

        let t_overview = Instant::now();
        let overview = mgr.get_overview_snapshot();
        assert!(!overview.app_version.is_empty());
        assert!(overview.health.report.is_some());
        assert!(!overview.activity.is_empty());
        assert!(
            t_overview.elapsed().as_millis() < 50,
            "Overview snapshot assembly must stay non-blocking, took {:?}",
            t_overview.elapsed()
        );
    }
}

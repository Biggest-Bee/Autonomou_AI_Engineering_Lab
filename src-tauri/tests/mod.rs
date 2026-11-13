// Integration tests for Autonomous AI Engineering Lab
// Tests security config, rate limiting, VAL, sandbox, and core functionality

#[cfg(test)]
mod tests {
    use autonomous_ai_engineering_lib::security::{SecurityConfig, RateLimiter};
    use autonomous_ai_engineering_lib::security::rate_limiter::{AuthTier, RateLimitConfig, RequestType};
    use autonomous_ai_engineering_lib::val::{ValDriver, MicroVMConfig, NetworkInterface};
    use autonomous_ai_engineering_lib::sandbox::WindowsSandbox;
    use autonomous_ai_engineering_lib::val::process_tracker::{ProcessTracker, ResourceLimits};

    #[test]
    fn test_security_config_environment_loading() {
        // Test that security config loads from environment variables
        let config = SecurityConfig::from_env().unwrap();

        // Verify defaults are set
        assert!(!config.system_prompt.is_empty());
        assert!(!config.default_terminal.is_empty());
    }

    #[tokio::test]
    async fn test_rate_limiter_github_api_limits() {
        // Deterministic unauthenticated tier: 10 search requests per minute
        let config = RateLimitConfig::from_tier(AuthTier::Unauthenticated);
        let rate_limiter = RateLimiter::with_config(config);

        // First 10 requests should be allowed
        for _ in 0..10 {
            let result = rate_limiter
                .check_rate_limit(RequestType::Search)
                .await
                .unwrap();
            assert!(result.allowed);
            rate_limiter.record_request(RequestType::Search).await;
        }

        // 11th request should be denied
        let result = rate_limiter
            .check_rate_limit(RequestType::Search)
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[test]
    fn test_val_driver_initialization() {
        let driver = ValDriver::new();
        assert!(driver.is_ok());

        let driver = driver.unwrap();
        assert!(driver.initialize().is_ok());
    }

    #[test]
    fn test_microvm_config_creation() {
        let config = MicroVMConfig {
            cpu_count: 2,
            memory_mb: 2048,
            boot_source: "test-kernel".to_string(),
            network_interfaces: vec![],
        };

        assert_eq!(config.cpu_count, 2);
        assert_eq!(config.memory_mb, 2048);
    }

    #[test]
    fn test_network_interface_creation() {
        let iface = NetworkInterface {
            interface_id: "eth0".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            ip_address: "10.0.0.2".to_string(),
        };

        assert_eq!(iface.interface_id, "eth0");
    }

    #[tokio::test]
    async fn test_process_tracker_creation() {
        let tracker = ProcessTracker::new();
        assert!(tracker.get_all_processes().await.is_empty());
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();

        assert_eq!(limits.max_cpu_percent, 100.0);
        assert_eq!(limits.max_memory_mb, 8192);
    }

    #[test]
    fn test_sandbox_engine_trait() {
        // Test that sandbox engine trait is implemented
        let sandbox = WindowsSandbox::new("test-distro");
        assert!(sandbox.is_ok());
    }
}

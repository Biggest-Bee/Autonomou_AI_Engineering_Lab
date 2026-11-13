// Integration tests for Autonomous AI Engineering Lab
// Tests security, VAL, sandbox, and core functionality

#[cfg(test)]
mod tests {
    use autonomous_ai_engineering_lib::security::{SecurityConfig, SecurityFilter, RateLimiter, ViolationTracker};
    use autonomous_ai_engineering_lib::security::rate_limiter::{AuthTier, RateLimitConfig, RequestType};
    use autonomous_ai_engineering_lib::val::{ValDriver, MicroVMConfig, NetworkInterface};
    use autonomous_ai_engineering_lib::sandbox::{SandboxEngine, WindowsSandbox};
    use autonomous_ai_engineering_lib::val::process_tracker::{ProcessTracker, ResourceLimits};

    fn test_security_config() -> SecurityConfig {
        SecurityConfig {
            system_prompt_tag: "<AI_SYSTEM_BOUNDARY>".to_string(),
            system_prompt: "<AI_SYSTEM_BOUNDARY> test <AI_SYSTEM_BOUNDARY>".to_string(),
            security_string: "<SECURITY_BOUNDARY>".to_string(),
            display_tag: "system".to_string(),
            production_mode: false,
            default_terminal: "powershell".to_string(),
        }
    }

    #[test]
    fn test_security_config_environment_loading() {
        // Test that security config loads from environment variables
        let config = SecurityConfig::from_env().unwrap();

        // Verify default values are set
        assert!(!config.system_prompt_tag.is_empty());
        assert!(!config.system_prompt.is_empty());
        assert!(!config.security_string.is_empty());
        assert!(!config.display_tag.is_empty());
    }

    #[test]
    fn test_security_filter_system_prompt_removal() {
        let config = test_security_config();
        let filter = SecurityFilter::new(config.clone());

        let test_content = format!("Some content {} Some more content", config.system_prompt_tag);
        let filtered = filter.filter_user_message(&test_content).unwrap();

        // Verify system prompt tag is removed
        assert!(!filtered.contains(&config.system_prompt_tag));
        assert!(filtered.contains("Some content"));
    }

    #[test]
    fn test_security_filter_response_leakage_detection() {
        let config = test_security_config();
        let filter = SecurityFilter::new(config);

        let test_response = "Normal response";
        let result = filter.check_response_leakage(test_response).unwrap();

        // Normal response should be safe
        assert!(result.is_safe);

        // Response leaking the system prompt tag should be flagged
        let leaky = filter
            .check_response_leakage("oops <AI_SYSTEM_BOUNDARY> leaked")
            .unwrap();
        assert!(!leaky.is_safe);
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

    #[tokio::test]
    async fn test_violation_tracker_initialization() {
        let tracker = ViolationTracker::new();

        // Test that tracker can report violation counts
        let user_id = "test_user";
        assert_eq!(tracker.get_violation_count(user_id).await.unwrap(), 0);
    }
}

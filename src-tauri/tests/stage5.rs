// Stage 5 integration tests: logging/monitoring, performance, users, audit, i18n

#[cfg(test)]
mod stage5_tests {
    use autonomous_ai_engineering_lib::logging::{LogEntry, LogLevel, LoggingManager};
    use autonomous_ai_engineering_lib::performance::{
        CacheStats, PerformanceMonitor, TtlCache, run_benchmark,
    };
    use autonomous_ai_engineering_lib::user_mgmt::{Permission, Role, UserManager};
    use autonomous_ai_engineering_lib::audit::{AuditEventType, AuditFilter, AuditLogger, AuditSeverity};
    use autonomous_ai_engineering_lib::i18n::{DetectionMethod, LanguageManager};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    // ---------- Logging & Monitoring ----------

    #[tokio::test]
    async fn test_logging_levels_and_filtering() {
        let mgr = LoggingManager::new(LogLevel::Info, 100);
        mgr.log(LogEntry::new(LogLevel::Debug, "t", "debug msg")).await;
        mgr.log(LogEntry::new(LogLevel::Info, "t", "info msg")).await;
        mgr.log(LogEntry::new(LogLevel::Error, "t", "error msg")).await;

        // Debug below min_level -> not buffered
        let all = mgr.query_logs(None, 100).await;
        assert_eq!(all.len(), 2);

        // Filter by Warn+
        let warns = mgr.query_logs(Some(LogLevel::Warn), 100).await;
        assert_eq!(warns.len(), 1);
        assert!(matches!(warns[0].level, LogLevel::Error));

        // Newest first ordering
        assert_eq!(warns[0].message, "error msg");
    }

    #[tokio::test]
    async fn test_logging_ring_buffer_eviction() {
        let mgr = LoggingManager::new(LogLevel::Trace, 5);
        for i in 0..10 {
            mgr.log(LogEntry::new(LogLevel::Info, "t", &format!("msg {}", i))).await;
        }
        let entries = mgr.query_logs(None, 100).await;
        assert_eq!(entries.len(), 5);
        // Newest is msg 9
        assert_eq!(entries[0].message, "msg 9");
        // Oldest retained is msg 5
        assert_eq!(entries[4].message, "msg 5");

        let stats = mgr.get_stats().await;
        assert_eq!(stats.dropped_entries, 5);
        assert_eq!(stats.total_logged, 10);
    }

    #[tokio::test]
    async fn test_logging_metrics() {
        let mgr = LoggingManager::new(LogLevel::Info, 100);
        mgr.increment_counter("requests", 5).await;
        mgr.increment_counter("requests", 3).await;
        mgr.increment_counter("errors", 1).await;
        mgr.set_gauge("cpu", 42.5).await;
        mgr.observe_duration("op", 25.0).await;
        mgr.observe_duration("op", 75.0).await;

        let counters = mgr.get_counters().await;
        let req = counters.iter().find(|c| c.name == "requests").unwrap();
        assert_eq!(req.value, 8);

        let gauges = mgr.get_gauges().await;
        assert!(gauges.iter().any(|g| g.name == "cpu" && g.value == 42.5));

        let hists = mgr.get_histograms().await;
        let op = hists.iter().find(|h| h.name == "op").unwrap();
        assert_eq!(op.count, 2);
        assert_eq!(op.min, 25.0);
        assert_eq!(op.max, 75.0);

        // Health checks run without panic
        let health = mgr.run_health_checks().await;
        assert!(!health.is_empty());
        assert!(health.iter().all(|h| h.healthy));
    }

    #[tokio::test]
    async fn test_logging_log_entry_fields() {
        let entry = LogEntry::new(LogLevel::Warn, "auth", "login failed")
            .with_field("user", serde_json::json!("alice"))
            .with_field("ip", serde_json::json!("1.2.3.4"));
        assert_eq!(entry.fields["user"], "alice");
        assert_eq!(entry.fields["ip"], "1.2.3.4");
    }

    // ---------- Performance ----------

    #[tokio::test]
    async fn test_ttl_cache_basic() {
        let cache: TtlCache<String> = TtlCache::new(Duration::from_secs(60), 10);
        cache.insert("k1", "v1".to_string(), None).await;
        assert_eq!(cache.get("k1").await, Some("v1".to_string()));
        assert_eq!(cache.get("missing").await, None);
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_ttl_cache_expiry() {
        let cache: TtlCache<i32> = TtlCache::new(Duration::from_millis(50), 10);
        cache.insert("k", 42, Some(Duration::from_millis(50))).await;
        assert_eq!(cache.get("k").await, Some(42));
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(cache.get("k").await, None);
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_ttl_cache_eviction_and_stats() {
        let cache: TtlCache<u32> = TtlCache::new(Duration::from_secs(60), 3);
        cache.insert("a", 1, None).await;
        cache.insert("b", 2, None).await;
        cache.insert("c", 3, None).await;
        cache.insert("d", 4, None).await; // triggers eviction

        let len = cache.len().await;
        assert!(len <= 3);

        // "d" was just inserted and is guaranteed present
        assert!(cache.get("d").await.is_some());
        cache.get("missing").await;
        let stats = cache.stats();
        assert!(stats.hits >= 1);
        assert!(stats.misses >= 1);
        let hit_rate = CacheStats { hits: stats.hits, misses: stats.misses }.hit_rate();
        assert!(hit_rate > 0.0 && hit_rate <= 1.0);
    }

    #[tokio::test]
    async fn test_ttl_cache_get_or_insert() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let cache: TtlCache<String> = TtlCache::new(Duration::from_secs(60), 10);
        let calls = Arc::new(AtomicU64::new(0));

        let c1 = calls.clone();
        let v1 = cache
            .get_or_insert_with("key", move || {
                let c = c1.clone();
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Ok("computed".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v1, "computed");
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        // Second call hits cache, doesn't recompute
        let c2 = calls.clone();
        let v2 = cache
            .get_or_insert_with("key", move || {
                let c = c2.clone();
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Ok("recomputed".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v2, "computed");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_performance_monitor_timing() {
        let mon = PerformanceMonitor::new(100);
        mon.time("fast_op", || async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            Ok(())
        })
        .await
        .unwrap();
        mon.time("fast_op", || async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        })
        .await
        .unwrap();
        mon.time("fail_op", || async {
            Err::<(), _>(anyhow::anyhow!("fail"))
        })
        .await
        .unwrap_err();

        let agg = mon.get_aggregate("fast_op").await.unwrap();
        assert_eq!(agg.count, 2);
        assert_eq!(agg.errors, 0);
        assert!(agg.total_ms > 0.0);
        let computed_avg = agg.total_ms / agg.count as f64;
        assert!(computed_avg > 0.0);

        let fail_agg = mon.get_aggregate("fail_op").await.unwrap();
        assert_eq!(fail_agg.errors, 1);

        let avg = mon.avg_duration_ms("fast_op").await.unwrap();
        assert!(avg > 0.0);

        let slowest = mon.get_slowest_operations(5).await;
        assert!(!slowest.is_empty());
    }

    #[tokio::test]
    async fn test_benchmark() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let counter = Arc::new(AtomicU64::new(0));
        let c = counter.clone();
        let result = run_benchmark("counter_bench", 100, move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        })
        .await
        .unwrap();

        assert_eq!(result.iterations, 100);
        assert_eq!(counter.load(Ordering::Relaxed), 100);
        assert!(result.throughput_ops_per_sec > 0.0);
    }

    // ---------- User Management ----------

    #[tokio::test]
    async fn test_user_create_and_get() {
        let mgr = UserManager::new();
        let res = mgr.create_user("alice", "alice@example.com", Role::Developer).await.unwrap();
        assert!(res.success);
        let user = res.user.unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.role, Role::Developer);

        assert!(mgr.get_user(&user.user_id).await.is_some());
        assert!(mgr.get_user_by_username("alice").await.is_some());
        assert_eq!(mgr.user_count().await, 1);
    }

    #[tokio::test]
    async fn test_user_duplicate_username_rejected() {
        let mgr = UserManager::new();
        mgr.create_user("bob", "b@x.com", Role::Viewer).await.unwrap();
        let res = mgr.create_user("bob", "b2@x.com", Role::Admin).await.unwrap();
        assert!(!res.success);
        assert!(res.message.contains("already exists"));
    }

    #[tokio::test]
    async fn test_role_permissions() {
        let viewer_perms = Permission::default_for_role(Role::Viewer);
        assert!(viewer_perms.contains(&Permission::ProjectRead));
        assert!(!viewer_perms.contains(&Permission::ProjectWrite));
        assert!(!viewer_perms.contains(&Permission::ManageUsers));

        let admin_perms = Permission::default_for_role(Role::Admin);
        assert!(admin_perms.contains(&Permission::ManageUsers));
        assert!(admin_perms.contains(&Permission::ShutdownSystem));
        assert!(admin_perms.contains(&Permission::ExecutePrivileged));

        // Role ordering
        assert!(Role::Admin > Role::Developer);
        assert!(Role::Developer > Role::Viewer);
    }

    #[tokio::test]
    async fn test_user_permission_checks() {
        let mgr = UserManager::new();
        let res = mgr.create_user("carol", "c@x.com", Role::Developer).await.unwrap();
        let uid = res.user.unwrap().user_id;

        // Developer can write projects and execute code
        assert!(mgr.check_permission(&uid, &Permission::ProjectWrite).await);
        assert!(mgr.check_permission(&uid, &Permission::ExecuteCode).await);
        // But not manage users
        assert!(!mgr.check_permission(&uid, &Permission::ManageUsers).await);

        // require_permission succeeds/fails appropriately
        assert!(mgr.require_permission(&uid, &Permission::ProjectRead).await.is_ok());
        assert!(mgr.require_permission(&uid, &Permission::ManageUsers).await.is_err());

        // Unknown user fails
        assert!(!mgr.check_permission("ghost", &Permission::ProjectRead).await);
    }

    #[tokio::test]
    async fn test_user_extra_and_denied_permissions() {
        let mgr = UserManager::new();
        let res = mgr.create_user("dave", "d@x.com", Role::Viewer).await.unwrap();
        let uid = res.user.unwrap().user_id;

        // Viewer lacks ProjectWrite by default
        assert!(!mgr.check_permission(&uid, &Permission::ProjectWrite).await);

        // Grant it explicitly
        mgr.grant_permission(&uid, Permission::ProjectWrite).await.unwrap();
        assert!(mgr.check_permission(&uid, &Permission::ProjectWrite).await);

        // Now deny ProjectRead (role default) -> overrides
        mgr.deny_permission(&uid, Permission::ProjectRead).await.unwrap();
        assert!(!mgr.check_permission(&uid, &Permission::ProjectRead).await);
    }

    #[tokio::test]
    async fn test_user_deactivation_blocks_access() {
        let mgr = UserManager::new();
        let res = mgr.create_user("erin", "e@x.com", Role::Admin).await.unwrap();
        let uid = res.user.unwrap().user_id;

        assert!(mgr.check_permission(&uid, &Permission::ManageUsers).await);
        mgr.set_active(&uid, false).await.unwrap();
        // Deactivated user loses all permissions
        assert!(!mgr.check_permission(&uid, &Permission::ManageUsers).await);
    }

    #[tokio::test]
    async fn test_user_role_change_and_delete() {
        let mgr = UserManager::new();
        let res = mgr.create_user("frank", "f@x.com", Role::Viewer).await.unwrap();
        let uid = res.user.unwrap().user_id;

        mgr.set_role(&uid, Role::Maintainer).await.unwrap();
        assert!(mgr.check_role(&uid, Role::Maintainer).await);
        assert!(mgr.check_permission(&uid, &Permission::ProjectDelete).await);

        mgr.record_login(&uid).await.unwrap();
        let user = mgr.get_user(&uid).await.unwrap();
        assert!(user.last_login_at.is_some());

        assert!(mgr.delete_user(&uid).await.unwrap());
        assert!(mgr.get_user(&uid).await.is_none());
        assert!(!mgr.delete_user(&uid).await.unwrap());
    }

    // ---------- Audit ----------

    #[tokio::test]
    async fn test_audit_record_and_query() {
        let logger = AuditLogger::new(1000);
        logger
            .record(
                AuditEventType::LoginSuccess,
                AuditSeverity::Info,
                "user1",
                "User logged in",
                Some("session_1".to_string()),
                HashMap::new(),
                Some("10.0.0.1".to_string()),
            )
            .await
            .unwrap();
        logger
            .log_event(AuditEventType::PermissionDenied, "user2", "Denied project_delete")
            .await
            .unwrap();

        // Query all
        let all = logger.query(&AuditFilter::default()).await;
        assert_eq!(all.len(), 2);

        // Filter by actor
        let user1_events = logger
            .query(&AuditFilter { actor: Some("user1".to_string()), ..Default::default() })
            .await;
        assert_eq!(user1_events.len(), 1);
        assert_eq!(user1_events[0].source_ip.as_deref(), Some("10.0.0.1"));

        // Filter by event type
        let denied = logger
            .query(&AuditFilter {
                event_types: Some(vec![AuditEventType::PermissionDenied]),
                ..Default::default()
            })
            .await;
        assert_eq!(denied.len(), 1);
        // PermissionDenied auto-severity is Warning
        assert_eq!(denied[0].severity, AuditSeverity::Warning);

        // Sequence numbers increment
        assert_eq!(all[0].sequence, 1); // newest first -> seq 1 is second record? check both
        let seqs: Vec<u64> = all.iter().map(|r| r.sequence).collect();
        assert!(seqs.contains(&0));
        assert!(seqs.contains(&1));
    }

    #[tokio::test]
    async fn test_audit_hash_chain_integrity() {
        let logger = AuditLogger::new(1000);
        for i in 0..5 {
            logger
                .log_event(
                    AuditEventType::CodeExecuted,
                    "agent",
                    &format!("Executed task {}", i),
                )
                .await
                .unwrap();
        }

        assert!(logger.verify_chain().await);

        // Tamper: modify a record in the buffer directly is not possible via API,
        // but verify_chain recomputes hashes so manual tampering would fail.
        // We verify prev_hash linkage is consistent.
        let records = logger.export_all().await;
        for w in records.windows(2) {
            assert_eq!(w[1].prev_hash, w[0].record_hash);
        }
    }

    #[tokio::test]
    async fn test_audit_stats() {
        let logger = AuditLogger::new(1000);
        logger.log_event(AuditEventType::LoginSuccess, "u", "login").await.unwrap();
        logger.log_event(AuditEventType::LoginFailure, "u", "bad login").await.unwrap();
        logger.log_event(AuditEventType::InjectionAttempt, "u", "jailbreak").await.unwrap();

        let stats = logger.get_stats().await;
        assert_eq!(stats.total_records, 3);
        assert!(stats.by_event_type.contains_key("LoginSuccess"));
        assert!(stats.oldest_timestamp.is_some());
        assert!(stats.newest_timestamp.is_some());
    }

    #[tokio::test]
    async fn test_audit_severity_assignment() {
        let logger = AuditLogger::new(100);
        // Critical events
        let r = logger
            .log_event(AuditEventType::RateLimitExceeded, "u", "limited")
            .await
            .unwrap();
        assert_eq!(r.severity, AuditSeverity::Critical);

        let r = logger
            .log_event(AuditEventType::SuspiciousActivity, "u", "suspicious")
            .await
            .unwrap();
        assert_eq!(r.severity, AuditSeverity::Critical);

        // Info default
        let r = logger
            .log_event(AuditEventType::FileRead, "u", "read file")
            .await
            .unwrap();
        assert_eq!(r.severity, AuditSeverity::Info);
    }

    // ---------- i18n / Multi-language ----------

    #[tokio::test]
    async fn test_language_initialization() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        let langs = mgr.list_languages().await;
        assert!(langs.len() >= 20);

        let enabled = mgr.list_enabled_languages().await;
        assert!(!enabled.is_empty());
    }

    #[tokio::test]
    async fn test_language_detection_by_extension() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        let d = mgr.detect_language("/project/src/main.rs").await;
        assert_eq!(d.language_id.as_deref(), Some("rust"));
        assert_eq!(d.confidence, 1.0);
        assert!(matches!(d.method, DetectionMethod::Extension));

        let d = mgr.detect_language("app.tsx").await;
        assert_eq!(d.language_id.as_deref(), Some("typescript"));

        let d = mgr.detect_language("script.py").await;
        assert_eq!(d.language_id.as_deref(), Some("python"));

        let d = mgr.detect_language("unknown.xyz123").await;
        assert!(d.language_id.is_none());
    }

    #[tokio::test]
    async fn test_language_detection_filename_and_shebang() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        // Filename-based (Dockerfile has no extension)
        let d = mgr.detect_language("/app/Dockerfile").await;
        assert_eq!(d.language_id.as_deref(), Some("dockerfile"));
        assert!(matches!(d.method, DetectionMethod::Filename));

        // Shebang detection
        let d = mgr
            .detect_language_with_content("script", Some("#!/usr/bin/env python3\nprint('hi')"))
            .await;
        assert_eq!(d.language_id.as_deref(), Some("python"));
        assert!(matches!(d.method, DetectionMethod::Shebang));

        let d = mgr
            .detect_language_with_content("run", Some("#!/bin/bash\necho hi"))
            .await;
        assert_eq!(d.language_id.as_deref(), Some("shell"));
    }

    #[tokio::test]
    async fn test_language_tree_sitter_availability() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        assert!(mgr.has_tree_sitter_grammar("rust").await);
        assert!(mgr.has_tree_sitter_grammar("python").await);
        // PowerShell has no grammar registered
        assert!(!mgr.has_tree_sitter_grammar("powershell").await);
        assert!(!mgr.has_tree_sitter_grammar("nonexistent").await);
    }

    #[tokio::test]
    async fn test_language_enable_disable() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        let before = mgr.list_enabled_languages().await.len();
        assert!(mgr.set_language_enabled("ruby", false).await.unwrap());
        let after = mgr.list_enabled_languages().await.len();
        assert_eq!(after, before - 1);
        assert!(!mgr.get_language("ruby").await.unwrap().enabled);
        assert!(mgr.set_language_enabled("ghost", false).await.unwrap() == false);
    }

    #[tokio::test]
    async fn test_i18n_translation() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        // Default locale is English
        assert_eq!(mgr.get_locale().await, "en");
        assert_eq!(mgr.translate("common.save").await, "Save");

        // Switch to Spanish
        mgr.set_locale("es").await;
        assert_eq!(mgr.translate("common.save").await, "Guardar");
        assert_eq!(mgr.translate("menu.file").await, "Archivo");

        // Missing key falls back to English, then to key itself
        assert_eq!(mgr.translate("nonexistent.key").await, "nonexistent.key");

        // Available locales
        let locales = mgr.available_locales().await;
        assert!(locales.contains(&"en".to_string()));
        assert!(locales.contains(&"es".to_string()));

        // Add a custom translation
        mgr.add_translation("fr", "greeting", "Bonjour").await;
        mgr.set_locale("fr").await;
        assert_eq!(mgr.translate("greeting").await, "Bonjour");
        // fr missing 'common.save' -> falls back to en
        assert_eq!(mgr.translate("common.save").await, "Save");
    }

    #[tokio::test]
    async fn test_custom_language_registration() {
        let mgr = Arc::new(LanguageManager::new());
        mgr.initialize().await.unwrap();

        let custom = autonomous_ai_engineering_lib::i18n::LanguageInfo {
            language_id: "customlang".to_string(),
            name: "CustomLang".to_string(),
            extensions: vec!["cl".to_string()],
            tree_sitter_grammar: None,
            comment_syntax: autonomous_ai_engineering_lib::i18n::CommentSyntax {
                line: "//".to_string(),
                block_start: None,
                block_end: None,
            },
            category: autonomous_ai_engineering_lib::i18n::LanguageCategory::Other,
            enabled: true,
        };
        mgr.register_language(custom).await.unwrap();

        let d = mgr.detect_language("file.cl").await;
        assert_eq!(d.language_id.as_deref(), Some("customlang"));
    }
}

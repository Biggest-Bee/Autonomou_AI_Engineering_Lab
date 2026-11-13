// Stage 4 integration tests: execution graph, autonomy, verification, mock API, LSP

#[cfg(test)]
mod stage4_tests {
    use autonomous_ai_engineering_lib::execution_graph::{
        EdgeType, ExecutionEdge, ExecutionGraphManager, ExecutionNode, GraphStatus, NodeStatus,
        NodeType,
    };
    use autonomous_ai_engineering_lib::autonomy::AutonomyCalculator;
    use autonomous_ai_engineering_lib::verification::{
        CheckStatus, CheckType, VerificationManager, VerificationStatus,
    };
    use autonomous_ai_engineering_lib::mock_api::{
        HttpMethod, MockAPIService, MockEndpoint, MockRequest,
    };
    use autonomous_ai_engineering_lib::lsp::{LSPClient, LSPConfig, LSPPosition};
    use std::collections::HashMap;

    fn test_node(id: &str) -> ExecutionNode {
        ExecutionNode {
            id: id.to_string(),
            node_type: NodeType::CodeGeneration,
            status: NodeStatus::Pending,
            dependencies: vec![],
            started_at: None,
            completed_at: None,
            duration_ms: None,
            error: None,
            metadata: serde_json::json!({}),
        }
    }

    // ---------- Execution Graph ----------

    #[tokio::test]
    async fn test_execution_graph_create_and_add() {
        let mgr = ExecutionGraphManager::new();
        mgr.initialize().await.unwrap();

        let graph = mgr.create_graph("g1".to_string()).await.unwrap();
        assert_eq!(graph.graph_id, "g1");
        assert!(matches!(graph.overall_status, GraphStatus::Pending));

        mgr.add_node("g1", test_node("n1")).await.unwrap();
        mgr.add_node("g1", test_node("n2")).await.unwrap();
        mgr.add_edge(
            "g1",
            ExecutionEdge {
                from: "n1".to_string(),
                to: "n2".to_string(),
                edge_type: EdgeType::DependsOn,
            },
        )
        .await
        .unwrap();

        let fetched = mgr.get_graph("g1").await.unwrap();
        assert_eq!(fetched.nodes.len(), 2);
        assert_eq!(fetched.edges.len(), 1);
    }

    #[tokio::test]
    async fn test_execution_graph_missing_graph_errors() {
        let mgr = ExecutionGraphManager::new();
        assert!(mgr.get_graph("nope").await.is_err());
        assert!(mgr.add_node("nope", test_node("n")).await.is_err());
    }

    #[tokio::test]
    async fn test_execution_graph_node_status_update() {
        let mgr = ExecutionGraphManager::new();
        mgr.create_graph("g2".to_string()).await.unwrap();
        mgr.add_node("g2", test_node("n1")).await.unwrap();

        mgr.update_node_status("g2", "n1", NodeStatus::Running)
            .await
            .unwrap();

        let graph = mgr.get_graph("g2").await.unwrap();
        assert!(matches!(graph.nodes["n1"].status, NodeStatus::Running));

        // Missing node errors
        assert!(mgr
            .update_node_status("g2", "missing", NodeStatus::Completed)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_execution_graph_execution_order() {
        let mgr = ExecutionGraphManager::new();
        mgr.create_graph("g3".to_string()).await.unwrap();
        // n1 -> n2 -> n3 chain
        let mut n2 = test_node("n2");
        n2.dependencies = vec!["n1".to_string()];
        let mut n3 = test_node("n3");
        n3.dependencies = vec!["n2".to_string()];

        mgr.add_node("g3", test_node("n1")).await.unwrap();
        mgr.add_node("g3", n2).await.unwrap();
        mgr.add_node("g3", n3).await.unwrap();
        mgr.add_edge(
            "g3",
            ExecutionEdge {
                from: "n1".to_string(),
                to: "n2".to_string(),
                edge_type: EdgeType::DependsOn,
            },
        )
        .await
        .unwrap();
        mgr.add_edge(
            "g3",
            ExecutionEdge {
                from: "n2".to_string(),
                to: "n3".to_string(),
                edge_type: EdgeType::DependsOn,
            },
        )
        .await
        .unwrap();

        let order = mgr.get_execution_order("g3").await.unwrap();
        assert_eq!(order.len(), 3);
        // n1 must come before n2, n2 before n3
        let pos = |id: &str| order.iter().position(|x| x == id).unwrap();
        assert!(pos("n1") < pos("n2"));
        assert!(pos("n2") < pos("n3"));
    }

    #[tokio::test]
    async fn test_execution_graph_stats_and_active() {
        let mgr = ExecutionGraphManager::new();
        mgr.create_graph("g4".to_string()).await.unwrap();
        mgr.add_node("g4", test_node("n1")).await.unwrap();

        let stats = mgr.get_graph_stats("g4").await.unwrap();
        assert_eq!(stats.total_nodes, 1);

        mgr.set_active_graph("sess1".to_string(), "g4".to_string())
            .await
            .unwrap();
        assert_eq!(mgr.get_active_graph("sess1").await, Some("g4".to_string()));
        assert_eq!(mgr.get_active_graph("unknown").await, None);
    }

    // ---------- Autonomy ----------

    #[tokio::test]
    async fn test_autonomy_session_lifecycle() {
        let calc = AutonomyCalculator::new();
        calc.initialize().await.unwrap();

        calc.start_session("s1".to_string()).await.unwrap();
        calc.record_autonomous_action("s1", "code_gen").await.unwrap();
        calc.record_autonomous_action("s1", "refactor").await.unwrap();
        calc.record_self_correction("s1", "fix_syntax").await.unwrap();
        calc.record_task_completion("s1", true, 100, 20, 5000)
            .await
            .unwrap();
        calc.record_task_completion("s1", false, 10, 5, 2000)
            .await
            .unwrap();

        let metrics = calc.get_metrics("s1").await.unwrap();
        assert_eq!(metrics.autonomous_actions, 2);
        assert_eq!(metrics.self_corrections, 1);
        assert_eq!(metrics.total_tasks, 2);
        assert_eq!(metrics.completed_tasks, 1);
        assert_eq!(metrics.failed_tasks, 1);
        assert_eq!(metrics.total_lines_added, 110);
    }

    #[tokio::test]
    async fn test_autonomy_score_calculation() {
        let calc = AutonomyCalculator::new();
        calc.start_session("s2".to_string()).await.unwrap();

        // Highly autonomous session
        for _ in 0..20 {
            calc.record_autonomous_action("s2", "action").await.unwrap();
        }
        for _ in 0..5 {
            calc.record_self_correction("s2", "correction").await.unwrap();
        }
        calc.record_task_completion("s2", true, 500, 100, 10000)
            .await
            .unwrap();

        let score = calc.calculate_score("s2").await.unwrap();
        assert!(score.total_score > 0.0);
        assert!(score.total_score <= 1.0);
        assert_eq!(score.session_id, "s2");

        // Score is retrievable afterwards
        let stored = calc.get_score("s2").await.unwrap();
        assert_eq!(stored.total_score, score.total_score);
    }

    #[tokio::test]
    async fn test_autonomy_unknown_session_errors() {
        let calc = AutonomyCalculator::new();
        assert!(calc.record_autonomous_action("ghost", "x").await.is_err());
        assert!(calc.calculate_score("ghost").await.is_err());
        assert!(calc.get_metrics("ghost").await.is_err());
    }

    #[tokio::test]
    async fn test_autonomy_leaderboard_ordering() {
        let calc = AutonomyCalculator::new();
        calc.start_session("low".to_string()).await.unwrap();
        calc.start_session("high".to_string()).await.unwrap();

        // "high" gets lots of autonomous activity
        for _ in 0..50 {
            calc.record_autonomous_action("high", "a").await.unwrap();
        }
        calc.record_task_completion("high", true, 1000, 0, 5000)
            .await
            .unwrap();
        // "low" gets almost nothing
        calc.record_task_completion("low", false, 1, 1, 60000)
            .await
            .unwrap();

        calc.calculate_score("low").await.unwrap();
        calc.calculate_score("high").await.unwrap();

        let board = calc.get_leaderboard().await;
        assert_eq!(board.len(), 2);
        assert_eq!(board[0].session_id, "high");
        assert!(board[0].total_score >= board[1].total_score);
    }

    // ---------- Verification ----------

    #[tokio::test]
    async fn test_verification_run_system_check() {
        let mgr = VerificationManager::new();
        mgr.initialize().await.unwrap();

        let report = mgr.run_system_check().await.unwrap();
        assert_eq!(report.summary.total_checks, 3);
        assert_eq!(report.summary.passed_checks, 3);
        assert_eq!(report.summary.failed_checks, 0);
        assert!(matches!(report.overall_status, VerificationStatus::Passed));
        assert!(report.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_verification_unregistered_check_skipped() {
        let mgr = VerificationManager::new();
        let report = mgr
            .run_verification(
                "r1".to_string(),
                vec![CheckType::Performance, CheckType::Integration],
            )
            .await
            .unwrap();

        // No executors registered for these -> all skipped -> still "Passed" (0 failures)
        assert_eq!(report.summary.skipped_checks, 2);
        assert!(matches!(report.overall_status, VerificationStatus::Passed));
    }

    #[tokio::test]
    async fn test_verification_custom_executor_failure() {
        let mgr = VerificationManager::new();
        mgr.register_check_executor(
            CheckType::Custom("failing".to_string()),
            std::sync::Arc::new(|| Err(anyhow::anyhow!("boom"))),
        )
        .await
        .unwrap();

        let report = mgr
            .run_verification("r2".to_string(), vec![CheckType::Custom("failing".to_string())])
            .await
            .unwrap();

        assert_eq!(report.summary.failed_checks, 1);
        assert!(!report.summary.critical_failures.is_empty());
        assert!(matches!(
            report.overall_status,
            VerificationStatus::Failed | VerificationStatus::Partial
        ));
        assert!(matches!(
            report.checks[0].status,
            CheckStatus::Failed(_)
        ));
    }

    #[tokio::test]
    async fn test_verification_report_retrieval() {
        let mgr = VerificationManager::new();
        mgr.run_system_check().await.unwrap();

        let all = mgr.get_all_reports().await;
        assert_eq!(all.len(), 1);

        let fetched = mgr.get_report(&all[0].report_id).await.unwrap();
        assert_eq!(fetched.report_id, all[0].report_id);
        assert!(mgr.get_report("missing").await.is_err());
    }

    // ---------- Mock API ----------

    #[tokio::test]
    async fn test_mock_api_default_endpoints() {
        let svc = MockAPIService::new();
        svc.initialize().await.unwrap();

        let endpoints = svc.list_endpoints().await;
        assert!(endpoints.len() >= 3);
        assert!(svc.get_endpoint("health").await.is_some());

        let stats = svc.get_stats().await;
        assert_eq!(stats.total_endpoints, endpoints.len());
    }

    #[tokio::test]
    async fn test_mock_api_handle_request() {
        let svc = MockAPIService::new();

        let resp = svc
            .handle_request(MockRequest {
                method: HttpMethod::GET,
                path: "/health".to_string(),
                headers: HashMap::new(),
                body: None,
                query_params: HashMap::new(),
            })
            .await
            .unwrap();

        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["status"], "healthy");
        assert!(resp.headers.contains_key("X-Mock-API"));
    }

    #[tokio::test]
    async fn test_mock_api_unknown_endpoint_404() {
        let svc = MockAPIService::new();

        let resp = svc
            .handle_request(MockRequest {
                method: HttpMethod::GET,
                path: "/does/not/exist".to_string(),
                headers: HashMap::new(),
                body: None,
                query_params: HashMap::new(),
            })
            .await
            .unwrap();

        assert_eq!(resp.status_code, 404);
    }

    #[tokio::test]
    async fn test_mock_api_register_unregister_and_disable() {
        let svc = MockAPIService::new();

        let custom = MockEndpoint {
            endpoint_id: "custom".to_string(),
            path: "/custom".to_string(),
            method: HttpMethod::POST,
            response: serde_json::json!({"ok": true}),
            status_code: 201,
            delay_ms: 0,
            enabled: true,
        };
        svc.register_endpoint(custom).await.unwrap();
        assert!(svc.get_endpoint("custom").await.is_some());

        let resp = svc
            .handle_request(MockRequest {
                method: HttpMethod::POST,
                path: "/custom".to_string(),
                headers: HashMap::new(),
                body: Some(serde_json::json!({"x": 1})),
                query_params: HashMap::new(),
            })
            .await
            .unwrap();
        assert_eq!(resp.status_code, 201);

        // Disable -> 404
        svc.enable_endpoint("custom", false).await.unwrap();
        let resp2 = svc
            .handle_request(MockRequest {
                method: HttpMethod::POST,
                path: "/custom".to_string(),
                headers: HashMap::new(),
                body: None,
                query_params: HashMap::new(),
            })
            .await
            .unwrap();
        assert_eq!(resp2.status_code, 404);

        // Request log recorded both requests
        let log = svc.get_request_log().await;
        assert_eq!(log.len(), 2);
        svc.clear_request_log().await.unwrap();
        assert!(svc.get_request_log().await.is_empty());

        svc.unregister_endpoint("custom").await.unwrap();
        assert!(svc.get_endpoint("custom").await.is_none());
        assert!(svc.enable_endpoint("ghost", true).await.is_err());
    }

    // ---------- LSP ----------

    #[tokio::test]
    async fn test_lsp_client_config_and_construction() {
        let config = LSPConfig {
            language: "rust".to_string(),
            server_command: "rust-analyzer".to_string(),
            server_args: vec![],
            workspace_root: "C:/proj".to_string(),
            enabled: true,
        };
        let client = LSPClient::new(config);
        // Construction succeeds without spawning a process
        drop(client);
    }

    #[tokio::test]
    async fn test_lsp_initialize_with_invalid_command_fails() {
        let config = LSPConfig {
            language: "rust".to_string(),
            server_command: "definitely-not-a-real-lsp-server-xyz".to_string(),
            server_args: vec![],
            workspace_root: "C:/proj".to_string(),
            enabled: true,
        };
        let mut client = LSPClient::new(config);
        // Spawning a nonexistent binary must fail gracefully
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_lsp_position_serialization() {
        let pos = LSPPosition {
            line: 10,
            character: 5,
        };
        let json = serde_json::to_string(&pos).unwrap();
        let back: LSPPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.line, 10);
        assert_eq!(back.character, 5);
    }
}

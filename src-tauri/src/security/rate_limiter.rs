// GitHub API Rate Limiter
// Implements GitHub API rate limiting with authentication tiers

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use anyhow::Result;

/// Rate limit tier based on authentication type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthTier {
    Unauthenticated,
    PersonalToken,
    GitHubApp,
}

impl AuthTier {
    pub fn from_env() -> Self {
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            if token.starts_with("ghp_") {
                AuthTier::PersonalToken
            } else if token.starts_with("ghs_") {
                AuthTier::GitHubApp
            } else {
                AuthTier::PersonalToken
            }
        } else {
            AuthTier::Unauthenticated
        }
    }
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub auth_tier: AuthTier,
    pub search_rate_limit: u32,    // requests per minute
    pub primary_rate_limit: u32,   // requests per hour
    pub secondary_rate_limit: u32, // points per minute
    pub content_creation_limit: u32, // requests per minute
    pub concurrency_limit: u32,     // concurrent requests
}

impl RateLimitConfig {
    pub fn from_tier(tier: AuthTier) -> Self {
        match tier {
            AuthTier::Unauthenticated => Self {
                auth_tier: tier,
                search_rate_limit: 10,      // 10 requests per minute
                primary_rate_limit: 60,     // 60 requests per hour
                secondary_rate_limit: 900, // 900 points per minute
                content_creation_limit: 80,
                concurrency_limit: 100,
            },
            AuthTier::PersonalToken => Self {
                auth_tier: tier,
                search_rate_limit: 30,      // 30 requests per minute
                primary_rate_limit: 5000,   // 5,000 requests per hour
                secondary_rate_limit: 900, // 900 points per minute
                content_creation_limit: 80,
                concurrency_limit: 100,
            },
            AuthTier::GitHubApp => Self {
                auth_tier: tier,
                search_rate_limit: 30,      // 30 requests per minute
                primary_rate_limit: 15000,  // 15,000 requests per hour
                secondary_rate_limit: 2000, // 2,000 points per minute
                content_creation_limit: 80,
                concurrency_limit: 100,
            },
        }
    }
}

/// Request type for specific rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestType {
    Search,
    Primary,
    Secondary,
    ContentCreation,
}

/// Rate limiter implementation
pub struct RateLimiter {
    config: RateLimitConfig,
    search_requests: Arc<RwLock<Vec<Instant>>>,
    primary_requests: Arc<RwLock<Vec<Instant>>>,
    secondary_points: Arc<RwLock<Vec<Instant>>>,
    content_creation_requests: Arc<RwLock<Vec<Instant>>>,
    concurrent_requests: Arc<RwLock<u32>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let tier = AuthTier::from_env();
        let config = RateLimitConfig::from_tier(tier);
        Self::with_config(config)
    }

    /// Create a rate limiter with an explicit configuration (useful for tests)
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            config,
            search_requests: Arc::new(RwLock::new(Vec::new())),
            primary_requests: Arc::new(RwLock::new(Vec::new())),
            secondary_points: Arc::new(RwLock::new(Vec::new())),
            content_creation_requests: Arc::new(RwLock::new(Vec::new())),
            concurrent_requests: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if a request can be made under rate limits
    pub async fn check_rate_limit(&self, request_type: RequestType) -> Result<RateLimitResult> {
        match request_type {
            RequestType::Search => {
                self.cleanup_old_requests(&self.search_requests, Duration::from_secs(60)).await;
                let requests = self.search_requests.read().await;
                
                if requests.len() >= self.config.search_rate_limit as usize {
                    return Ok(RateLimitResult {
                        allowed: false,
                        retry_after: Some(Duration::from_secs(60)),
                        reason: "Search rate limit exceeded".to_string(),
                    });
                }
            }
            RequestType::Primary => {
                self.cleanup_old_requests(&self.primary_requests, Duration::from_secs(3600)).await;
                let requests = self.primary_requests.read().await;
                
                if requests.len() >= self.config.primary_rate_limit as usize {
                    return Ok(RateLimitResult {
                        allowed: false,
                        retry_after: Some(Duration::from_secs(3600)),
                        reason: "Primary rate limit exceeded".to_string(),
                    });
                }
            }
            RequestType::Secondary => {
                self.cleanup_old_requests(&self.secondary_points, Duration::from_secs(60)).await;
                let points = self.secondary_points.read().await;
                
                if points.len() >= self.config.secondary_rate_limit as usize {
                    return Ok(RateLimitResult {
                        allowed: false,
                        retry_after: Some(Duration::from_secs(60)),
                        reason: "Secondary rate limit exceeded".to_string(),
                    });
                }
            }
            RequestType::ContentCreation => {
                self.cleanup_old_requests(&self.content_creation_requests, Duration::from_secs(60)).await;
                let requests = self.content_creation_requests.read().await;
                
                if requests.len() >= self.config.content_creation_limit as usize {
                    return Ok(RateLimitResult {
                        allowed: false,
                        retry_after: Some(Duration::from_secs(60)),
                        reason: "Content creation rate limit exceeded".to_string(),
                    });
                }
            }
        }

        // Check concurrency limit
        let concurrent = self.concurrent_requests.read().await;
        if *concurrent >= self.config.concurrency_limit {
            return Ok(RateLimitResult {
                allowed: false,
                retry_after: Some(Duration::from_millis(100)),
                reason: "Concurrency limit exceeded".to_string(),
            });
        }

        Ok(RateLimitResult {
            allowed: true,
            retry_after: None,
            reason: String::new(),
        })
    }

    /// Record a request has been made
    pub async fn record_request(&self, request_type: RequestType) {
        let now = Instant::now();

        match request_type {
            RequestType::Search => {
                let mut requests = self.search_requests.write().await;
                requests.push(now);
            }
            RequestType::Primary => {
                let mut requests = self.primary_requests.write().await;
                requests.push(now);
            }
            RequestType::Secondary => {
                let mut points = self.secondary_points.write().await;
                points.push(now);
            }
            RequestType::ContentCreation => {
                let mut requests = self.content_creation_requests.write().await;
                requests.push(now);
            }
        }
    }

    /// Start a concurrent request
    pub async fn start_concurrent(&self) {
        let mut concurrent = self.concurrent_requests.write().await;
        *concurrent += 1;
    }

    /// End a concurrent request
    pub async fn end_concurrent(&self) {
        let mut concurrent = self.concurrent_requests.write().await;
        *concurrent = concurrent.saturating_sub(1);
    }

    /// Clean up old requests beyond the time window
    async fn cleanup_old_requests(&self, requests: &Arc<RwLock<Vec<Instant>>>, window: Duration) {
        let now = Instant::now();
        let mut req = requests.write().await;
        req.retain(|&timestamp| now.duration_since(timestamp) < window);
    }

    /// Get current rate limit status
    pub async fn get_status(&self) -> RateLimitStatus {
        RateLimitStatus {
            auth_tier: self.config.auth_tier,
            search_remaining: self.config.search_rate_limit.saturating_sub(self.search_requests.read().await.len() as u32),
            primary_remaining: self.config.primary_rate_limit.saturating_sub(self.primary_requests.read().await.len() as u32),
            secondary_remaining: self.config.secondary_rate_limit.saturating_sub(self.secondary_points.read().await.len() as u32),
            content_creation_remaining: self.config.content_creation_limit.saturating_sub(self.content_creation_requests.read().await.len() as u32),
            concurrent_current: *self.concurrent_requests.read().await,
            concurrent_limit: self.config.concurrency_limit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub retry_after: Option<Duration>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub auth_tier: AuthTier,
    pub search_remaining: u32,
    pub primary_remaining: u32,
    pub secondary_remaining: u32,
    pub content_creation_remaining: u32,
    pub concurrent_current: u32,
    pub concurrent_limit: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
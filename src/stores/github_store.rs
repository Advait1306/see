use gpui::*;
use std::collections::HashMap;
use std::time::Duration;

use crate::config;
use crate::github::{
    CreateReviewRequest, GitHubClient, InstallationListResponse, InstallationReposResponse,
    PullRequest, PullRequestFile, Review, ReviewComment, StoredToken,
};

const GITHUB_CLIENT_ID: &str = "Iv23liZXdkklKaMCOedA";
const GITHUB_APP_ID: u64 = 2816951;
const GITHUB_APP_SLUG: &str = "august-see";
const POLL_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    Unauthenticated,
    WaitingForUser {
        user_code: String,
        verification_uri: String,
    },
    CheckingInstallation,
    NeedsInstallation {
        install_url: String,
    },
    Authenticated,
    NoAccess,
    Error(String),
}

#[derive(Clone)]
pub enum GitHubStoreEvent {
    AuthStateChanged,
    PullRequestsUpdated,
    PrDetailsUpdated(u64),
}

pub struct PrDetailCache {
    pub files: Vec<PullRequestFile>,
    pub comments: Vec<ReviewComment>,
    pub reviews: Vec<Review>,
}

pub struct GitHubStore {
    owner: String,
    repo: String,
    auth_state: AuthState,
    token: Option<String>,
    pull_requests: Vec<PullRequest>,
    pr_details: HashMap<u64, PrDetailCache>,
    _poll_task: Option<Task<()>>,
    _auth_poll_task: Option<Task<()>>,
}

impl EventEmitter<GitHubStoreEvent> for GitHubStore {}

impl GitHubStore {
    pub fn new(owner: String, repo: String, cx: &mut Context<Self>) -> Self {
        let mut store = Self {
            owner,
            repo,
            auth_state: AuthState::Unauthenticated,
            token: None,
            pull_requests: Vec::new(),
            pr_details: HashMap::new(),
            _poll_task: None,
            _auth_poll_task: None,
        };

        if let Some(stored) = Self::load_token() {
            store.token = Some(stored.access_token);
            store.auth_state = AuthState::CheckingInstallation;
            store.check_installation(cx);
        }

        store
    }

    fn load_token() -> Option<StoredToken> {
        let path = config::github_token_path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_token(token: &str) {
        let stored = StoredToken {
            access_token: token.to_string(),
        };
        config::save_json(&config::github_token_path(), &stored);
    }

    fn clear_token() {
        let path = config::github_token_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    pub fn auth_state(&self) -> &AuthState {
        &self.auth_state
    }

    pub fn pull_requests(&self) -> &[PullRequest] {
        &self.pull_requests
    }

    pub fn pr_details(&self, pr_number: u64) -> Option<&PrDetailCache> {
        self.pr_details.get(&pr_number)
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    fn check_installation(&mut self, cx: &mut Context<Self>) {
        let token = match &self.token {
            Some(t) => t.clone(),
            None => return,
        };
        let owner = self.owner.clone();
        let repo = self.repo.clone();

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let http = reqwest::blocking::Client::new();

                // Check what installations the user has for our app
                let resp = http
                    .get("https://api.github.com/user/installations")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Accept", "application/vnd.github+json")
                    .header("User-Agent", "august-app")
                    .send();

                let body = match resp {
                    Ok(r) => r.text().unwrap_or_default(),
                    Err(e) => return Err(e.to_string()),
                };

                let installations: InstallationListResponse = match serde_json::from_str(&body) {
                    Ok(i) => i,
                    Err(e) => {
                        log::error!("Failed to parse installations: {} body: {}", e, body);
                        return Err(format!("Failed to check installations: {}", e));
                    }
                };

                // Find our app's installation
                let our_installation = installations
                    .installations
                    .iter()
                    .find(|i| i.app_id == GITHUB_APP_ID);

                if let Some(installation) = our_installation {
                    // App is installed — check if this repo is accessible
                    let repos_resp = http
                        .get(format!(
                            "https://api.github.com/user/installations/{}/repositories",
                            installation.id
                        ))
                        .header("Authorization", format!("Bearer {}", token))
                        .header("Accept", "application/vnd.github+json")
                        .header("User-Agent", "august-app")
                        .send();

                    let repos_body = match repos_resp {
                        Ok(r) => r.text().unwrap_or_default(),
                        Err(_) => return Ok(true), // Assume accessible on network error
                    };

                    let repos: InstallationReposResponse =
                        match serde_json::from_str(&repos_body) {
                            Ok(r) => r,
                            Err(_) => return Ok(true),
                        };

                    let target = format!("{}/{}", owner, repo);
                    let has_access = repos.repositories.iter().any(|r| r.full_name == target);

                    if has_access {
                        Ok(true)
                    } else {
                        // Installed but doesn't include this repo
                        let install_url = format!(
                            "https://github.com/apps/{}/installations/new",
                            installation.app_slug
                        );
                        Err(install_url)
                    }
                } else {
                    Err(format!(
                        "https://github.com/apps/{}/installations/new",
                        GITHUB_APP_SLUG
                    ))
                }
            })
            .await;

            match result {
                Ok(true) => {
                    let _ = this.update(cx, |store, cx| {
                        store.auth_state = AuthState::Authenticated;
                        cx.emit(GitHubStoreEvent::AuthStateChanged);
                        cx.notify();
                        store.start_polling(cx);
                    });
                }
                Err(install_url) => {
                    let _ = this.update(cx, |store, cx| {
                        store.auth_state = AuthState::NeedsInstallation { install_url };
                        cx.emit(GitHubStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                }
                _ => {}
            }
        })
        .detach();
    }

    pub fn retry_installation_check(&mut self, cx: &mut Context<Self>) {
        self.auth_state = AuthState::CheckingInstallation;
        cx.emit(GitHubStoreEvent::AuthStateChanged);
        cx.notify();
        self.check_installation(cx);
    }

    pub fn start_device_flow(&mut self, cx: &mut Context<Self>) {
        self._auth_poll_task = Some(cx.spawn(async move |this, cx| {
            let device_code_result = smol::unblock(|| {
                let http = reqwest::blocking::Client::new();
                let resp = http
                    .post("https://github.com/login/device/code")
                    .header("Accept", "application/json")
                    .header("User-Agent", "august-app")
                    .form(&[("client_id", GITHUB_CLIENT_ID)])
                    .send();

                match resp {
                    Ok(r) => {
                        let status = r.status();
                        let body = r.text().unwrap_or_default();
                        if !status.is_success() {
                            log::error!(
                                "Device code request failed ({}): {}",
                                status,
                                body
                            );
                            return Err(body);
                        }
                        serde_json::from_str::<crate::github::DeviceCodeResponse>(&body)
                            .map_err(|e| {
                                log::error!(
                                    "Failed to parse device code response: {} body: {}",
                                    e,
                                    body
                                );
                                format!("JSON parse error: {}", e)
                            })
                    }
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

            let device_code_resp = match device_code_result {
                Ok(d) => d,
                Err(e) => {
                    let _ = this.update(cx, |store, cx| {
                        store.auth_state = AuthState::Error(e.to_string());
                        cx.emit(GitHubStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                    return;
                }
            };

            let _ = smol::unblock({
                let uri = device_code_resp.verification_uri.clone();
                move || open::that(&uri)
            })
            .await;

            let _ = this.update(cx, |store, cx| {
                store.auth_state = AuthState::WaitingForUser {
                    user_code: device_code_resp.user_code.clone(),
                    verification_uri: device_code_resp.verification_uri.clone(),
                };
                cx.emit(GitHubStoreEvent::AuthStateChanged);
                cx.notify();
            });

            let interval = Duration::from_secs(device_code_resp.interval.max(5));
            let device_code = device_code_resp.device_code;

            loop {
                cx.background_executor().timer(interval).await;

                let dc = device_code.clone();
                let token_result = smol::unblock(move || {
                    let http = reqwest::blocking::Client::new();
                    let resp = http
                        .post("https://github.com/login/oauth/access_token")
                        .header("Accept", "application/json")
                        .header("User-Agent", "august-app")
                        .form(&[
                            ("client_id", GITHUB_CLIENT_ID),
                            ("device_code", dc.as_str()),
                            (
                                "grant_type",
                                "urn:ietf:params:oauth:grant-type:device_code",
                            ),
                        ])
                        .send();

                    match resp {
                        Ok(r) => {
                            let body = r.text().unwrap_or_default();
                            serde_json::from_str::<crate::github::AccessTokenResponse>(&body)
                                .map_err(|e| {
                                    log::error!(
                                        "Failed to parse access token response: {} body: {}",
                                        e,
                                        body
                                    );
                                    e.to_string()
                                })
                        }
                        Err(e) => Err(e.to_string()),
                    }
                })
                .await;

                let token_resp = match token_result {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                if let Some(error) = &token_resp.error {
                    match error.as_str() {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            cx.background_executor()
                                .timer(Duration::from_secs(5))
                                .await;
                            continue;
                        }
                        "expired_token" | "access_denied" => {
                            let _ = this.update(cx, |store, cx| {
                                store.auth_state =
                                    AuthState::Error(format!("Auth failed: {}", error));
                                cx.emit(GitHubStoreEvent::AuthStateChanged);
                                cx.notify();
                            });
                            return;
                        }
                        _ => continue,
                    }
                }

                if !token_resp.access_token.is_empty() {
                    Self::save_token(&token_resp.access_token);
                    let _ = this.update(cx, |store, cx| {
                        store.token = Some(token_resp.access_token.clone());
                        store.auth_state = AuthState::CheckingInstallation;
                        store._auth_poll_task = None;
                        cx.emit(GitHubStoreEvent::AuthStateChanged);
                        cx.notify();
                        store.check_installation(cx);
                    });
                    return;
                }
            }
        }));
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        Self::clear_token();
        self.token = None;
        self.auth_state = AuthState::Unauthenticated;
        self.pull_requests.clear();
        self.pr_details.clear();
        self._poll_task = None;
        self._auth_poll_task = None;
        cx.emit(GitHubStoreEvent::AuthStateChanged);
        cx.notify();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner.clone();
        let repo = self.repo.clone();

        self.fetch_pull_requests(cx);

        self._poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(POLL_INTERVAL_SECS))
                    .await;

                let token = this.update(cx, |store, _| store.token.clone()).ok();
                let token = match token {
                    Some(Some(t)) => t,
                    _ => return,
                };

                let o = owner.clone();
                let r = repo.clone();
                let result = smol::unblock(move || {
                    let client = GitHubClient::new(token);
                    client.list_pull_requests(&o, &r)
                })
                .await;

                match result {
                    Ok(prs) => {
                        let _ = this.update(cx, |store, cx| {
                            store.pull_requests = prs;
                            cx.emit(GitHubStoreEvent::PullRequestsUpdated);
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        if e.status().is_some_and(|s| {
                            s == reqwest::StatusCode::FORBIDDEN
                                || s == reqwest::StatusCode::NOT_FOUND
                        }) {
                            let _ = this.update(cx, |store, cx| {
                                store.auth_state = AuthState::NoAccess;
                                store._poll_task = None;
                                cx.emit(GitHubStoreEvent::AuthStateChanged);
                                cx.notify();
                            });
                            return;
                        }
                        log::error!("Failed to fetch PRs: {}", e);
                    }
                }
            }
        }));
    }

    fn fetch_pull_requests(&mut self, cx: &mut Context<Self>) {
        let token = match &self.token {
            Some(t) => t.clone(),
            None => return,
        };
        let owner = self.owner.clone();
        let repo = self.repo.clone();

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let client = GitHubClient::new(token);
                client.list_pull_requests(&owner, &repo)
            })
            .await;

            match result {
                Ok(prs) => {
                    let _ = this.update(cx, |store, cx| {
                        store.pull_requests = prs;
                        cx.emit(GitHubStoreEvent::PullRequestsUpdated);
                        cx.notify();
                    });
                }
                Err(e) => {
                    if e.status().is_some_and(|s| {
                        s == reqwest::StatusCode::FORBIDDEN
                            || s == reqwest::StatusCode::NOT_FOUND
                    }) {
                        let _ = this.update(cx, |store, cx| {
                            store.auth_state = AuthState::NoAccess;
                            store._poll_task = None;
                            cx.emit(GitHubStoreEvent::AuthStateChanged);
                            cx.notify();
                        });
                    } else {
                        log::error!("Failed to fetch PRs: {}", e);
                    }
                }
            }
        })
        .detach();
    }

    pub fn load_pr_details(&mut self, pr_number: u64, cx: &mut Context<Self>) {
        let token = match &self.token {
            Some(t) => t.clone(),
            None => return,
        };
        let owner = self.owner.clone();
        let repo = self.repo.clone();

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let client = GitHubClient::new(token);
                let files = client.get_pull_request_files(&owner, &repo, pr_number)?;
                let comments = client.get_review_comments(&owner, &repo, pr_number)?;
                let reviews = client.get_reviews(&owner, &repo, pr_number)?;
                Ok::<_, reqwest::Error>((files, comments, reviews))
            })
            .await;

            if let Ok((files, comments, reviews)) = result {
                let _ = this.update(cx, |store, cx| {
                    store.pr_details.insert(
                        pr_number,
                        PrDetailCache {
                            files,
                            comments,
                            reviews,
                        },
                    );
                    cx.emit(GitHubStoreEvent::PrDetailsUpdated(pr_number));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    #[cfg(test)]
    pub fn set_test_state(
        &mut self,
        auth_state: AuthState,
        token: Option<String>,
        pull_requests: Vec<PullRequest>,
    ) {
        self.auth_state = auth_state;
        self.token = token;
        self.pull_requests = pull_requests;
    }

    #[cfg(test)]
    pub fn set_test_pr_details(&mut self, pr_number: u64, details: PrDetailCache) {
        self.pr_details.insert(pr_number, details);
    }

    pub fn submit_review(
        &mut self,
        pr_number: u64,
        request: CreateReviewRequest,
        cx: &mut Context<Self>,
    ) {
        let token = match &self.token {
            Some(t) => t.clone(),
            None => return,
        };
        let owner = self.owner.clone();
        let repo = self.repo.clone();

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let client = GitHubClient::new(token);
                client.submit_review(&owner, &repo, pr_number, &request)
            })
            .await;

            match result {
                Ok(_review) => {
                    let _ = this.update(cx, |store, cx| {
                        store.load_pr_details(pr_number, cx);
                    });
                }
                Err(e) => {
                    log::error!("Failed to submit review: {}", e);
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::PullRequest;
    use crate::test_helpers;

    #[core::prelude::v1::test]
    fn test_initial_auth_state_unauthenticated() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            cx.read(|cx| {
                assert_eq!(store.read(cx).auth_state(), &AuthState::Unauthenticated);
                assert!(store.read(cx).pull_requests().is_empty());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_token_save_and_load_roundtrip() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            GitHubStore::save_token("test-token-123");

            let loaded = GitHubStore::load_token();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().access_token, "test-token-123");
        });
    }

    #[core::prelude::v1::test]
    fn test_sign_out_clears_state() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            store.update(cx, |store, _cx| {
                store.token = Some("test-token".to_string());
                store.auth_state = AuthState::Authenticated;
            });

            cx.read(|cx| {
                assert_eq!(store.read(cx).auth_state(), &AuthState::Authenticated);
            });

            store.update(cx, |store, cx| {
                store.sign_out(cx);
            });

            cx.read(|cx| {
                assert_eq!(store.read(cx).auth_state(), &AuthState::Unauthenticated);
                assert!(store.read(cx).token.is_none());
                assert!(store.read(cx).pull_requests().is_empty());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_authenticated_when_token_exists() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            GitHubStore::save_token("existing-token");

            let loaded = GitHubStore::load_token();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().access_token, "existing-token");

            GitHubStore::clear_token();
            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            cx.read(|cx| {
                assert_eq!(store.read(cx).auth_state(), &AuthState::Unauthenticated);
            });

            store.update(cx, |store, _cx| {
                store.token = Some("existing-token".to_string());
                store.auth_state = AuthState::Authenticated;
            });

            cx.read(|cx| {
                assert_eq!(store.read(cx).auth_state(), &AuthState::Authenticated);
                assert_eq!(store.read(cx).token.as_deref(), Some("existing-token"));
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_pr_details_cache_roundtrip() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            store.update(cx, |store, _cx| {
                store.pr_details.insert(
                    42,
                    PrDetailCache {
                        files: vec![],
                        comments: vec![],
                        reviews: vec![],
                    },
                );
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert!(s.pr_details(42).is_some());
                assert!(s.pr_details(42).unwrap().files.is_empty());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_pr_details_returns_none_for_unknown() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            cx.read(|cx| {
                assert!(store.read(cx).pr_details(999).is_none());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_sign_out_clears_pr_details() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            store.update(cx, |store, _cx| {
                store.pr_details.insert(
                    1,
                    PrDetailCache {
                        files: vec![],
                        comments: vec![],
                        reviews: vec![],
                    },
                );
            });

            store.update(cx, |store, cx| {
                store.sign_out(cx);
            });

            cx.read(|cx| {
                assert!(store.read(cx).pr_details(1).is_none());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_multiple_pr_details_independent() {
        test_helpers::run_gpui_test(|cx| {
            let _fixture = test_helpers::TestFixture::new(cx);

            let store = cx.new(|cx| GitHubStore::new("owner".into(), "repo".into(), cx));

            store.update(cx, |store, _cx| {
                store.pr_details.insert(
                    1,
                    PrDetailCache {
                        files: vec![],
                        comments: vec![],
                        reviews: vec![],
                    },
                );
                store.pr_details.insert(
                    2,
                    PrDetailCache {
                        files: vec![],
                        comments: vec![],
                        reviews: vec![],
                    },
                );
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert!(s.pr_details(1).is_some());
                assert!(s.pr_details(2).is_some());
                assert!(s.pr_details(3).is_none());
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_pr_deserialization() {
        let json = r#"[{
            "number": 42,
            "title": "Test PR",
            "body": "Description here",
            "state": "open",
            "draft": false,
            "user": {"login": "testuser", "avatar_url": "https://example.com/avatar.png"},
            "head": {"sha": "abc123", "ref": "feature-branch"},
            "base": {"sha": "def456", "ref": "main"},
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z"
        }]"#;

        let prs: Vec<PullRequest> = serde_json::from_str(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].title, "Test PR");
        assert_eq!(prs[0].user.login, "testuser");
        assert!(!prs[0].draft);
    }
}

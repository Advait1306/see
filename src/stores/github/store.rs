use crate::types::github::{AuthState, GitHubRepo, PullRequest, PullRequestState, RemoteAuthState};
use git2::Repository;
use gpui::{Context, EventEmitter, Subscription, Task};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::account_store::{GitHubAccountStore, GitHubAccountStoreEvent};
use super::http;
use super::remote::parse_github_remote;

#[derive(Clone)]
pub enum GitHubStoreEvent {
    RemotesDiscovered,
    PullRequestsUpdated,
    AuthStateChanged,
}

impl EventEmitter<GitHubStoreEvent> for GitHubStore {}

pub struct RemoteState {
    pub repo: GitHubRepo,
    pub auth_state: RemoteAuthState,
    pub pull_requests: Vec<PullRequest>,
}

pub struct GitHubStore {
    repository: Arc<Repository>,
    remotes: HashMap<String, RemoteState>,
    _poll_task: Option<Task<()>>,
    _account_subscription: Subscription,
}

impl GitHubStore {
    pub fn new(repository: &Arc<Repository>, cx: &mut Context<Self>) -> Self {
        let account_store = GitHubAccountStore::global(cx);
        let account_sub = cx.subscribe(&account_store, |this, _store, event, cx| match event {
            GitHubAccountStoreEvent::AuthStateChanged
            | GitHubAccountStoreEvent::InstallationsUpdated => {
                this.update_remote_auth_states(cx);
                this.fetch_pull_requests(cx);
                cx.emit(GitHubStoreEvent::AuthStateChanged);
            }
        });

        let mut store = Self {
            repository: repository.clone(),
            remotes: HashMap::new(),
            _poll_task: None,
            _account_subscription: account_sub,
        };

        store.discover_remotes(cx);
        store.update_remote_auth_states(cx);
        store.fetch_pull_requests(cx);

        store
    }

    pub fn with_polling(mut self, cx: &mut Context<Self>) -> Self {
        self._poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let _ = this.update(cx, |store, cx| {
                    store.fetch_pull_requests(cx);
                });
            }
        }));
        self
    }

    pub fn discover_remotes(&mut self, cx: &mut Context<Self>) {
        let remote_names = match self.repository.remotes() {
            Ok(remotes) => remotes
                .iter()
                .filter_map(|name| name.map(|n| n.to_string()))
                .collect::<Vec<_>>(),
            Err(_) => return,
        };

        let mut changed = false;
        for name in remote_names {
            if self.remotes.contains_key(&name) {
                continue;
            }

            let url = match self.repository.find_remote(&name) {
                Ok(remote) => remote.url().map(|u| u.to_string()),
                Err(_) => continue,
            };

            if let Some(url) = url {
                if let Some(repo) = parse_github_remote(&url) {
                    self.remotes.insert(
                        name,
                        RemoteState {
                            repo,
                            auth_state: RemoteAuthState::NotAuthenticated,
                            pull_requests: Vec::new(),
                        },
                    );
                    changed = true;
                }
            }
        }

        if changed {
            cx.emit(GitHubStoreEvent::RemotesDiscovered);
            cx.notify();
        }
    }

    pub fn update_remote_auth_states(&mut self, cx: &mut Context<Self>) {
        let account_store = GitHubAccountStore::global(cx);
        let account = account_store.read(cx);

        let signed_in = matches!(account.auth_state(), AuthState::SignedIn { .. });

        for remote in self.remotes.values_mut() {
            remote.auth_state = if !signed_in {
                RemoteAuthState::NotAuthenticated
            } else if account.is_installed_for_owner(&remote.repo.owner) {
                RemoteAuthState::Connected
            } else {
                RemoteAuthState::AppNotInstalled
            };
        }

        cx.notify();
    }

    pub fn fetch_pull_requests(&mut self, cx: &mut Context<Self>) {
        let account_store = GitHubAccountStore::global(cx);
        let account = account_store.read(cx);

        let token = match account.access_token() {
            Some(t) => t.to_string(),
            None => return,
        };

        let connected_remotes: Vec<(String, GitHubRepo)> = self
            .remotes
            .iter()
            .filter(|(_, state)| state.auth_state == RemoteAuthState::Connected)
            .map(|(name, state)| (name.clone(), state.repo.clone()))
            .collect();

        if connected_remotes.is_empty() {
            return;
        }

        cx.spawn(async move |this, cx| {
            let client = http::http_client();
            let handle = http::http_runtime().handle().clone();
            let mut all_prs: HashMap<String, Vec<PullRequest>> = HashMap::new();

            for (remote_name, repo) in &connected_remotes {
                let prs = {
                    let client = client.clone();
                    let token = token.clone();
                    let repo = repo.clone();
                    handle
                        .spawn(async move {
                            let url = format!(
                                "https://api.github.com/repos/{}/{}/pulls?state=open&per_page=50",
                                repo.owner, repo.repo
                            );

                            let resp = client
                                .get(&url)
                                .header("Authorization", format!("Bearer {}", token))
                                .header("User-Agent", "august-app")
                                .header("Accept", "application/vnd.github+json")
                                .send()
                                .await;

                            let resp = match resp {
                                Ok(r) if r.status().is_success() => r,
                                _ => return Vec::new(),
                            };

                            let body: serde_json::Value = match resp.json().await {
                                Ok(v) => v,
                                Err(_) => return Vec::new(),
                            };

                            let mut prs = Vec::new();
                            if let Some(items) = body.as_array() {
                                for item in items {
                                    if let Some(pr) = parse_pull_request(item, &repo) {
                                        prs.push(pr);
                                    }
                                }
                            }
                            prs
                        })
                        .await
                        .unwrap()
                };

                all_prs.insert(remote_name.clone(), prs);
            }

            let _ = this.update(cx, |store, cx| {
                let mut changed = false;
                for (remote_name, prs) in all_prs {
                    if let Some(remote) = store.remotes.get_mut(&remote_name) {
                        if remote.pull_requests.len() != prs.len()
                            || remote
                                .pull_requests
                                .iter()
                                .zip(prs.iter())
                                .any(|(a, b)| {
                                    a.number != b.number
                                        || a.title != b.title
                                        || a.updated_at != b.updated_at
                                })
                        {
                            remote.pull_requests = prs;
                            changed = true;
                        }
                    }
                }
                if changed {
                    cx.emit(GitHubStoreEvent::PullRequestsUpdated);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn all_pull_requests(&self) -> Vec<&PullRequest> {
        self.remotes
            .values()
            .flat_map(|state| state.pull_requests.iter())
            .collect()
    }

    pub fn remotes(&self) -> &HashMap<String, RemoteState> {
        &self.remotes
    }
}

fn parse_pull_request(item: &serde_json::Value, repo: &GitHubRepo) -> Option<PullRequest> {
    let number = item["number"].as_u64()?;
    let title = item["title"].as_str()?.to_string();
    let head_ref = item["head"]["ref"].as_str()?.to_string();
    let base_ref = item["base"]["ref"].as_str()?.to_string();
    let author_login = item["user"]["login"].as_str().unwrap_or("unknown").to_string();
    let draft = item["draft"].as_bool().unwrap_or(false);
    let html_url = item["html_url"].as_str()?.to_string();
    let created_at = item["created_at"].as_str().unwrap_or("").to_string();
    let updated_at = item["updated_at"].as_str().unwrap_or("").to_string();

    let state_str = item["state"].as_str().unwrap_or("open");
    let merged = item["merged_at"].is_string();
    let state = if merged {
        PullRequestState::Merged
    } else if state_str == "closed" {
        PullRequestState::Closed
    } else {
        PullRequestState::Open
    };

    Some(PullRequest {
        number,
        title,
        state,
        head_ref,
        base_ref,
        author_login,
        draft,
        html_url,
        repo: repo.clone(),
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::run_gpui_test;
    use gpui::AppContext as _;

    #[test]
    fn test_discover_remotes_from_repo() {
        run_gpui_test(|cx| {
            let tmp = tempfile::TempDir::new().unwrap();
            let repo = Repository::init(tmp.path()).unwrap();

            // Add a GitHub remote
            repo.remote("origin", "git@github.com:testowner/testrepo.git")
                .unwrap();

            let _fixture = crate::test_helpers::TestFixture::new(cx);
            cx.update(|cx| {
                crate::stores::TerminalStore::init(cx);
                crate::stores::WorkspaceStore::init(cx);
                GitHubAccountStore::init(cx);
            });

            let arc_repo = Arc::new(repo);
            let store = cx.new(|cx| GitHubStore::new(&arc_repo, cx));

            cx.read(|cx| {
                let s = store.read(cx);
                assert_eq!(s.remotes().len(), 1);
                let remote = s.remotes().get("origin").unwrap();
                assert_eq!(remote.repo.owner, "testowner");
                assert_eq!(remote.repo.repo, "testrepo");
            });
        });
    }

    #[test]
    fn test_discover_remotes_ignores_non_github() {
        run_gpui_test(|cx| {
            let tmp = tempfile::TempDir::new().unwrap();
            let repo = Repository::init(tmp.path()).unwrap();

            repo.remote("origin", "git@gitlab.com:testowner/testrepo.git")
                .unwrap();

            let _fixture = crate::test_helpers::TestFixture::new(cx);
            cx.update(|cx| {
                crate::stores::TerminalStore::init(cx);
                crate::stores::WorkspaceStore::init(cx);
                GitHubAccountStore::init(cx);
            });

            let arc_repo = Arc::new(repo);
            let store = cx.new(|cx| GitHubStore::new(&arc_repo, cx));

            cx.read(|cx| {
                assert!(store.read(cx).remotes().is_empty());
            });
        });
    }

    #[test]
    fn test_all_pull_requests_empty_when_no_prs() {
        run_gpui_test(|cx| {
            let tmp = tempfile::TempDir::new().unwrap();
            let repo = Repository::init(tmp.path()).unwrap();
            repo.remote("origin", "git@github.com:testowner/testrepo.git")
                .unwrap();

            let _fixture = crate::test_helpers::TestFixture::new(cx);
            cx.update(|cx| {
                crate::stores::TerminalStore::init(cx);
                crate::stores::WorkspaceStore::init(cx);
                GitHubAccountStore::init(cx);
            });

            let arc_repo = Arc::new(repo);
            let store = cx.new(|cx| GitHubStore::new(&arc_repo, cx));

            cx.read(|cx| {
                assert!(store.read(cx).all_pull_requests().is_empty());
            });
        });
    }
}

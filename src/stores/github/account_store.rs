use crate::types::github::{AuthState, DeviceCodeResponse, Installation, TokenResponse};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Task};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use super::auth;
use super::http;

#[derive(Clone)]
pub enum GitHubAccountStoreEvent {
    AuthStateChanged,
    InstallationsUpdated,
}

impl EventEmitter<GitHubAccountStoreEvent> for GitHubAccountStore {}

pub struct GitHubAccountStore {
    auth_state: AuthState,
    access_token: Option<String>,
    installations: HashMap<String, Installation>,
    _poll_task: Option<Task<()>>,
}

struct GlobalGitHubAccountStore(Entity<GitHubAccountStore>);

impl Global for GlobalGitHubAccountStore {}

impl GitHubAccountStore {
    pub fn init(cx: &mut App) {
        let store = cx.new(|cx| {
            let mut store = Self {
                auth_state: AuthState::SignedOut,
                access_token: None,
                installations: HashMap::new(),
                _poll_task: None,
            };
            store.try_restore_session(cx);
            store
        });
        cx.set_global(GlobalGitHubAccountStore(store));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalGitHubAccountStore>().0.clone()
    }

    pub fn auth_state(&self) -> &AuthState {
        &self.auth_state
    }

    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    pub fn installation_for_owner(&self, owner: &str) -> Option<&Installation> {
        self.installations.get(&owner.to_lowercase())
    }

    pub fn is_installed_for_owner(&self, owner: &str) -> bool {
        self.installations.contains_key(&owner.to_lowercase())
    }

    pub fn sign_in(&mut self, cx: &mut Context<Self>) {
        if matches!(self.auth_state, AuthState::Authenticating { .. }) {
            return;
        }

        self._poll_task = Some(cx.spawn(async move |this, cx| {
            let client = http::http_client();
            let handle = http::http_runtime().handle().clone();

            // Step 1: Request device code
            let device_code_response = {
                let client = client.clone();
                handle
                    .spawn(async move {
                        let resp = client
                            .post(auth::DEVICE_CODE_URL)
                            .header("Accept", "application/json")
                            .form(&[("client_id", auth::GITHUB_CLIENT_ID)])
                            .send()
                            .await?;
                        resp.json::<DeviceCodeResponse>().await
                    })
                    .await
                    .unwrap()
            };

            let device_code_response = match device_code_response {
                Ok(dcr) => dcr,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.auth_state =
                            AuthState::Error(format!("Failed to get device code: {}", e));
                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                    return;
                }
            };

            // Step 2: Update state with user code and open browser
            let device_code = device_code_response.device_code.clone();
            let mut interval = device_code_response.interval;
            let verification_uri = device_code_response.verification_uri.clone();

            let _ = this.update(cx, |this, cx| {
                this.auth_state = AuthState::Authenticating {
                    user_code: device_code_response.user_code.clone(),
                    verification_uri: device_code_response.verification_uri.clone(),
                };
                cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                cx.notify();
            });

            let _ = Command::new("open").arg(&verification_uri).spawn();

            // Step 3: Poll for token
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(interval))
                    .await;

                let body = {
                    let client = client.clone();
                    let dc = device_code.clone();
                    handle
                        .spawn(async move {
                            let resp = client
                                .post(auth::TOKEN_URL)
                                .header("Accept", "application/json")
                                .form(&[
                                    ("client_id", auth::GITHUB_CLIENT_ID),
                                    ("device_code", &dc),
                                    (
                                        "grant_type",
                                        "urn:ietf:params:oauth:grant-type:device_code",
                                    ),
                                ])
                                .send()
                                .await;
                            match resp {
                                Ok(r) => r.text().await.ok(),
                                Err(_) => None,
                            }
                        })
                        .await
                        .unwrap()
                };

                let body = match body {
                    Some(b) => b,
                    None => continue,
                };

                if body.contains("authorization_pending") {
                    continue;
                }
                if body.contains("slow_down") {
                    interval += 5;
                    continue;
                }
                if body.contains("expired_token") {
                    let _ = this.update(cx, |this, cx| {
                        this.auth_state =
                            AuthState::Error("Device code expired. Please try again.".into());
                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                    return;
                }
                if body.contains("access_denied") {
                    let _ = this.update(cx, |this, cx| {
                        this.auth_state = AuthState::Error("Access denied.".into());
                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                    return;
                }

                if let Ok(token_response) = serde_json::from_str::<TokenResponse>(&body) {
                    auth::save_token_to_keychain(&token_response.access_token);

                    let (username, installations) = {
                        let client = client.clone();
                        let token = token_response.access_token.clone();
                        handle
                            .spawn(async move {
                                let username = fetch_username(&client, &token).await;
                                let installations = fetch_installations(&client, &token).await;
                                (username, installations)
                            })
                            .await
                            .unwrap()
                    };

                    let _ = this.update(cx, |this, cx| {
                        this.access_token = Some(token_response.access_token);
                        this.auth_state = AuthState::SignedIn {
                            username: username.unwrap_or_else(|| "unknown".into()),
                        };

                        this.installations.clear();
                        for inst in installations {
                            this.installations
                                .insert(inst.account_login.to_lowercase(), inst);
                        }

                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.emit(GitHubAccountStoreEvent::InstallationsUpdated);
                        cx.notify();
                    });
                    return;
                }
            }
        }));
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        auth::delete_token_from_keychain();
        self.access_token = None;
        self.auth_state = AuthState::SignedOut;
        self.installations.clear();
        self._poll_task = None;
        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
        cx.emit(GitHubAccountStoreEvent::InstallationsUpdated);
        cx.notify();
    }

    fn try_restore_session(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = auth::load_token_from_keychain() {
            self.access_token = Some(token.clone());

            cx.spawn(async move |this, cx| {
                let client = http::http_client();
                let handle = http::http_runtime().handle().clone();

                let username = {
                    let client = client.clone();
                    let token = token.clone();
                    handle
                        .spawn(async move { fetch_username(&client, &token).await })
                        .await
                        .unwrap()
                };

                if let Some(username) = username {
                    let installations = {
                        let client = client.clone();
                        let token = token.clone();
                        handle
                            .spawn(async move { fetch_installations(&client, &token).await })
                            .await
                            .unwrap()
                    };

                    let _ = this.update(cx, |this, cx| {
                        this.auth_state = AuthState::SignedIn { username };

                        this.installations.clear();
                        for inst in installations {
                            this.installations
                                .insert(inst.account_login.to_lowercase(), inst);
                        }

                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.emit(GitHubAccountStoreEvent::InstallationsUpdated);
                        cx.notify();
                    });
                } else {
                    let _ = this.update(cx, |this, cx| {
                        auth::delete_token_from_keychain();
                        this.access_token = None;
                        this.auth_state = AuthState::SignedOut;
                        cx.emit(GitHubAccountStoreEvent::AuthStateChanged);
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }
}

async fn fetch_username(client: &reqwest::Client, token: &str) -> Option<String> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "august-app")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    body["login"].as_str().map(|s| s.to_string())
}

async fn fetch_installations(client: &reqwest::Client, token: &str) -> Vec<Installation> {
    let resp = client
        .get("https://api.github.com/user/installations")
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

    let mut installations = Vec::new();
    if let Some(items) = body["installations"].as_array() {
        for item in items {
            if let (Some(id), Some(login), Some(account_type)) = (
                item["id"].as_u64(),
                item["account"]["login"].as_str(),
                item["account"]["type"].as_str(),
            ) {
                installations.push(Installation {
                    id,
                    account_login: login.to_string(),
                    account_type: account_type.to_string(),
                });
            }
        }
    }

    installations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::run_gpui_test;

    #[test]
    fn test_initial_state_is_signed_out() {
        run_gpui_test(|cx| {
            let store = cx.new(|_cx| GitHubAccountStore {
                auth_state: AuthState::SignedOut,
                access_token: None,
                installations: HashMap::new(),
                _poll_task: None,
            });

            cx.read(|cx| {
                assert_eq!(*store.read(cx).auth_state(), AuthState::SignedOut);
                assert!(store.read(cx).access_token().is_none());
            });
        });
    }

    #[test]
    fn test_sign_out_clears_state() {
        run_gpui_test(|cx| {
            let store = cx.new(|_cx| {
                let mut s = GitHubAccountStore {
                    auth_state: AuthState::SignedIn {
                        username: "testuser".into(),
                    },
                    access_token: Some("fake-token".into()),
                    installations: HashMap::new(),
                    _poll_task: None,
                };
                s.installations.insert(
                    "myorg".into(),
                    Installation {
                        id: 1,
                        account_login: "myorg".into(),
                        account_type: "Organization".into(),
                    },
                );
                s
            });

            store.update(cx, |store, cx| {
                store.sign_out(cx);
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert_eq!(*s.auth_state(), AuthState::SignedOut);
                assert!(s.access_token().is_none());
                assert!(!s.is_installed_for_owner("myorg"));
            });
        });
    }

    #[test]
    fn test_installation_lookup() {
        run_gpui_test(|cx| {
            let store = cx.new(|_cx| {
                let mut s = GitHubAccountStore {
                    auth_state: AuthState::SignedIn {
                        username: "testuser".into(),
                    },
                    access_token: Some("fake-token".into()),
                    installations: HashMap::new(),
                    _poll_task: None,
                };
                s.installations.insert(
                    "myorg".into(),
                    Installation {
                        id: 123,
                        account_login: "myorg".into(),
                        account_type: "Organization".into(),
                    },
                );
                s
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert!(s.is_installed_for_owner("myorg"));
                assert!(s.is_installed_for_owner("MyOrg")); // case insensitive
                assert!(!s.is_installed_for_owner("other-org"));

                let inst = s.installation_for_owner("myorg").unwrap();
                assert_eq!(inst.id, 123);
                assert_eq!(inst.account_login, "myorg");
            });
        });
    }
}

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAuthState {
    NotAuthenticated,
    AppNotInstalled,
    Connected,
}

#[derive(Debug, Clone)]
pub struct Installation {
    pub id: u64,
    pub account_login: String,
    pub account_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: PullRequestState,
    pub head_ref: String,
    pub base_ref: String,
    pub author_login: String,
    pub draft: bool,
    pub html_url: String,
    pub repo: GitHubRepo,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    SignedOut,
    Authenticating {
        user_code: String,
        verification_uri: String,
    },
    SignedIn {
        username: String,
    },
    Error(String),
}

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

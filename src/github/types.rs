use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRef {
    pub sha: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub draft: bool,
    pub user: GitHubUser,
    pub head: GitRef,
    pub base: GitRef,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestFile {
    pub sha: String,
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changes: u64,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: u64,
    pub body: String,
    pub path: String,
    pub position: Option<u64>,
    pub line: Option<u64>,
    pub side: Option<String>,
    pub user: GitHubUser,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub user: GitHubUser,
    pub body: Option<String>,
    pub state: String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewComment {
    pub path: String,
    pub body: String,
    pub line: u64,
    pub side: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewRequest {
    pub body: String,
    pub event: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CreateReviewComment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenResponse {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Installation {
    pub id: u64,
    pub app_id: u64,
    pub app_slug: String,
    pub account: GitHubUser,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallationListResponse {
    pub total_count: u64,
    pub installations: Vec<Installation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallationRepo {
    pub full_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallationReposResponse {
    pub total_count: u64,
    pub repositories: Vec<InstallationRepo>,
}

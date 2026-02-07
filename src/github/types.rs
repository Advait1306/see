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

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn test_review_comment_deserialization() {
        let json = r#"{
            "id": 100,
            "body": "Great change",
            "path": "src/main.rs",
            "position": 5,
            "line": 42,
            "side": "RIGHT",
            "user": {"login": "reviewer", "avatar_url": "https://example.com/a.png"},
            "created_at": "2024-03-01T10:00:00Z"
        }"#;
        let comment: ReviewComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, 100);
        assert_eq!(comment.line, Some(42));
        assert_eq!(comment.side.as_deref(), Some("RIGHT"));
        assert_eq!(comment.position, Some(5));
        assert_eq!(comment.user.login, "reviewer");
    }

    #[core::prelude::v1::test]
    fn test_review_comment_missing_optional_fields() {
        let json = r#"{
            "id": 101,
            "body": "Comment without line info",
            "path": "README.md",
            "position": null,
            "line": null,
            "side": null,
            "user": {"login": "user1", "avatar_url": "https://example.com/b.png"},
            "created_at": "2024-03-02T10:00:00Z"
        }"#;
        let comment: ReviewComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.line, None);
        assert_eq!(comment.side, None);
        assert_eq!(comment.position, None);
    }

    #[core::prelude::v1::test]
    fn test_review_deserialization() {
        let json = r#"{
            "id": 200,
            "user": {"login": "reviewer", "avatar_url": "https://example.com/a.png"},
            "body": null,
            "state": "APPROVED",
            "submitted_at": null
        }"#;
        let review: Review = serde_json::from_str(json).unwrap();
        assert_eq!(review.id, 200);
        assert_eq!(review.body, None);
        assert_eq!(review.state, "APPROVED");
        assert_eq!(review.submitted_at, None);
    }

    #[core::prelude::v1::test]
    fn test_pull_request_file_deserialization() {
        let json_with_patch = r#"{
            "sha": "abc123",
            "filename": "src/lib.rs",
            "status": "modified",
            "additions": 10,
            "deletions": 3,
            "changes": 13,
            "patch": "@@ -1,3 +1,4 @@\n context"
        }"#;
        let file: PullRequestFile = serde_json::from_str(json_with_patch).unwrap();
        assert_eq!(file.filename, "src/lib.rs");
        assert!(file.patch.is_some());

        let json_no_patch = r#"{
            "sha": "def456",
            "filename": "binary.bin",
            "status": "added",
            "additions": 0,
            "deletions": 0,
            "changes": 0
        }"#;
        let file2: PullRequestFile = serde_json::from_str(json_no_patch).unwrap();
        assert_eq!(file2.patch, None);
    }

    #[core::prelude::v1::test]
    fn test_create_review_request_serialization() {
        let request = CreateReviewRequest {
            body: "LGTM".to_string(),
            event: "APPROVE".to_string(),
            comments: Vec::new(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"event\":\"APPROVE\""));
        assert!(json.contains("\"body\":\"LGTM\""));
        // Empty comments should be skipped due to skip_serializing_if
        assert!(!json.contains("comments"));
    }

    #[core::prelude::v1::test]
    fn test_create_review_request_with_comments() {
        let request = CreateReviewRequest {
            body: "".to_string(),
            event: "COMMENT".to_string(),
            comments: vec![CreateReviewComment {
                path: "src/main.rs".to_string(),
                body: "Fix this".to_string(),
                line: 10,
                side: "RIGHT".to_string(),
            }],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"comments\""));
        assert!(json.contains("\"path\":\"src/main.rs\""));
    }

    #[core::prelude::v1::test]
    fn test_device_code_response_deserialization() {
        let json = r#"{
            "device_code": "abc123",
            "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900,
            "interval": 5
        }"#;
        let resp: DeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.device_code, "abc123");
        assert_eq!(resp.user_code, "ABCD-1234");
        assert_eq!(resp.interval, 5);

        // Round-trip
        let serialized = serde_json::to_string(&resp).unwrap();
        let resp2: DeviceCodeResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp2.user_code, "ABCD-1234");
    }

    #[core::prelude::v1::test]
    fn test_access_token_response_defaults() {
        // Minimal JSON — all fields should get defaults
        let json = r#"{}"#;
        let resp: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "");
        assert_eq!(resp.token_type, "");
        assert_eq!(resp.scope, "");
        assert_eq!(resp.error, None);
    }
}

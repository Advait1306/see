use crate::types::github::TokenResponse;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serde::{Deserialize, Serialize};

pub const GITHUB_CLIENT_ID: &str = "Iv23liZXdkklKaMCOedA";
pub const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

const KEYCHAIN_SERVICE: &str = "com.august.github";
const KEYCHAIN_ACCOUNT: &str = "oauth-token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub fn save_credentials(access_token: &str, refresh_token: Option<&str>) {
    let creds = StoredCredentials {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.map(|s| s.to_string()),
    };
    let json = serde_json::to_string(&creds).unwrap();
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    if let Err(e) = set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, json.as_bytes()) {
        log::error!("Failed to save credentials to keychain: {}", e);
    }
}

pub fn load_credentials() -> Option<StoredCredentials> {
    let bytes = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).ok()?;
    let text = String::from_utf8(bytes.to_vec()).ok()?;

    // Try JSON format first (new)
    if let Ok(creds) = serde_json::from_str::<StoredCredentials>(&text) {
        return Some(creds);
    }

    // Fall back to plain token string (old format migration)
    if !text.is_empty() && !text.starts_with('{') {
        return Some(StoredCredentials {
            access_token: text,
            refresh_token: None,
        });
    }

    None
}

pub fn delete_credentials() {
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
}

pub async fn refresh_access_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Option<TokenResponse> {
    let resp = client
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    resp.json::<TokenResponse>().await.ok()
}

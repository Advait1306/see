use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

pub const GITHUB_CLIENT_ID: &str = "Iv23liZXdkklKaMCOedA";
pub const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

const KEYCHAIN_SERVICE: &str = "com.august.github";
const KEYCHAIN_ACCOUNT: &str = "oauth-token";

pub fn save_token_to_keychain(token: &str) {
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    if let Err(e) = set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, token.as_bytes()) {
        log::error!("Failed to save token to keychain: {}", e);
    }
}

pub fn load_token_from_keychain() -> Option<String> {
    match get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
        Ok(bytes) => String::from_utf8(bytes.to_vec()).ok(),
        Err(_) => None,
    }
}

pub fn delete_token_from_keychain() {
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
}

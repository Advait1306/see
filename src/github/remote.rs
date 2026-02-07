/// Parses a GitHub remote URL and returns (owner, repo).
///
/// Supports formats:
/// - `git@github.com:owner/repo.git`
/// - `ssh://git@github.com/owner/repo.git`
/// - `https://github.com/owner/repo.git`
/// - All of the above without `.git` suffix
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();

    // SSH format: git@github.com:owner/repo.git
    if let Some(path) = url.strip_prefix("git@github.com:") {
        return parse_owner_repo(path);
    }

    // SSH protocol: ssh://git@github.com/owner/repo.git
    if let Some(path) = url.strip_prefix("ssh://git@github.com/") {
        return parse_owner_repo(path);
    }

    // HTTPS: https://github.com/owner/repo.git
    if let Some(path) = url.strip_prefix("https://github.com/") {
        return parse_owner_repo(path);
    }

    // HTTP: http://github.com/owner/repo.git
    if let Some(path) = url.strip_prefix("http://github.com/") {
        return parse_owner_repo(path);
    }

    None
}

fn parse_owner_repo(path: &str) -> Option<(String, String)> {
    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_end_matches('/');
    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;

    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }

    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_with_git_suffix() {
        let result = parse_github_remote("git@github.com:owner/repo.git");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_ssh_without_git_suffix() {
        let result = parse_github_remote("git@github.com:owner/repo");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_https_with_git_suffix() {
        let result = parse_github_remote("https://github.com/owner/repo.git");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_https_without_git_suffix() {
        let result = parse_github_remote("https://github.com/owner/repo");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_ssh_protocol_url() {
        let result = parse_github_remote("ssh://git@github.com/owner/repo.git");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_http_url() {
        let result = parse_github_remote("http://github.com/owner/repo.git");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_non_github_url_returns_none() {
        assert_eq!(parse_github_remote("git@gitlab.com:owner/repo.git"), None);
        assert_eq!(parse_github_remote("https://gitlab.com/owner/repo"), None);
        assert_eq!(parse_github_remote("not-a-url"), None);
    }

    #[test]
    fn test_trailing_slash_stripped() {
        let result = parse_github_remote("https://github.com/owner/repo/");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }

    #[test]
    fn test_empty_owner_or_repo_returns_none() {
        assert_eq!(parse_github_remote("git@github.com:/repo.git"), None);
        assert_eq!(parse_github_remote("git@github.com:owner/"), None);
    }

    #[test]
    fn test_whitespace_trimmed() {
        let result = parse_github_remote("  git@github.com:owner/repo.git  ");
        assert_eq!(result, Some(("owner".into(), "repo".into())));
    }
}

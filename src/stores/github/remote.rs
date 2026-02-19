use crate::types::github::GitHubRepo;

pub fn parse_github_remote(url: &str) -> Option<GitHubRepo> {
    let url = url.trim();

    // SSH: git@github.com:owner/repo.git
    if let Some(path) = url.strip_prefix("git@github.com:") {
        return parse_owner_repo(path);
    }

    // HTTPS: https://github.com/owner/repo.git
    if let Ok(parsed) = url::Url::parse(url) {
        if parsed.host_str() != Some("github.com") {
            return None;
        }
        let path = parsed.path().trim_start_matches('/');
        return parse_owner_repo(path);
    }

    None
}

fn parse_owner_repo(path: &str) -> Option<GitHubRepo> {
    let path = path
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();

    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }

    Some(GitHubRepo { owner, repo })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_url() {
        let result = parse_github_remote("git@github.com:owner/repo.git");
        assert_eq!(
            result,
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "repo".into(),
            })
        );
    }

    #[test]
    fn test_parse_ssh_url_no_suffix() {
        let result = parse_github_remote("git@github.com:owner/repo");
        assert_eq!(
            result,
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "repo".into(),
            })
        );
    }

    #[test]
    fn test_parse_https_url() {
        let result = parse_github_remote("https://github.com/owner/repo.git");
        assert_eq!(
            result,
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "repo".into(),
            })
        );
    }

    #[test]
    fn test_parse_https_url_no_suffix() {
        let result = parse_github_remote("https://github.com/owner/repo");
        assert_eq!(
            result,
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "repo".into(),
            })
        );
    }

    #[test]
    fn test_parse_https_url_trailing_slash() {
        let result = parse_github_remote("https://github.com/owner/repo/");
        assert_eq!(
            result,
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "repo".into(),
            })
        );
    }

    #[test]
    fn test_non_github_url_returns_none() {
        assert_eq!(
            parse_github_remote("https://gitlab.com/owner/repo.git"),
            None
        );
    }

    #[test]
    fn test_invalid_url_returns_none() {
        assert_eq!(parse_github_remote("not a url"), None);
    }

    #[test]
    fn test_empty_url_returns_none() {
        assert_eq!(parse_github_remote(""), None);
    }

    #[test]
    fn test_ssh_missing_repo_returns_none() {
        assert_eq!(parse_github_remote("git@github.com:owner/"), None);
    }
}

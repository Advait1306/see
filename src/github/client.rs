use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{de::DeserializeOwned, Serialize};

use super::types::*;

const GITHUB_API_BASE: &str = "https://api.github.com";
const APP_USER_AGENT: &str = "august-app";

pub struct GitHubClient {
    http: Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        Self {
            http: Client::new(),
            token,
        }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, reqwest::Error> {
        self.http
            .get(format!("{}{}", GITHUB_API_BASE, path))
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/vnd.github+json")
            .header(USER_AGENT, APP_USER_AGENT)
            .send()?
            .error_for_status()?
            .json::<T>()
    }

    fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, reqwest::Error> {
        self.http
            .post(format!("{}{}", GITHUB_API_BASE, path))
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/vnd.github+json")
            .header(USER_AGENT, APP_USER_AGENT)
            .json(body)
            .send()?
            .error_for_status()?
            .json::<T>()
    }

    pub fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>, reqwest::Error> {
        self.get(&format!(
            "/repos/{}/{}/pulls?state=open&sort=updated&direction=desc",
            owner, repo
        ))
    }

    pub fn get_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PullRequestFile>, reqwest::Error> {
        self.get(&format!(
            "/repos/{}/{}/pulls/{}/files",
            owner, repo, pr_number
        ))
    }

    pub fn get_review_comments(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<ReviewComment>, reqwest::Error> {
        self.get(&format!(
            "/repos/{}/{}/pulls/{}/comments",
            owner, repo, pr_number
        ))
    }

    pub fn get_reviews(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<Review>, reqwest::Error> {
        self.get(&format!(
            "/repos/{}/{}/pulls/{}/reviews",
            owner, repo, pr_number
        ))
    }

    pub fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        request: &CreateReviewRequest,
    ) -> Result<Review, reqwest::Error> {
        self.post(
            &format!("/repos/{}/{}/pulls/{}/reviews", owner, repo, pr_number),
            request,
        )
    }
}

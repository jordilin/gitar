use crate::{
    http::Request,
    remote::{Member, MergeRequestResponse, Project},
    time::Seconds,
    Result,
};
use regex::Regex;
use serde::Serialize;
use std::{collections::HashMap, ffi::OsStr};

pub trait Runner {
    type Response;
    fn run<T>(&self, cmd: T) -> Result<Self::Response>
    where
        T: IntoIterator,
        T::Item: AsRef<OsStr>;
}

pub trait HttpRunner {
    type Response;
    fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response>;
    /// Return the number of API MAX PAGES allowed for the given Request.
    fn api_max_pages<T: Serialize>(&self, cmd: &Request<T>) -> u32;
}

#[derive(Clone, Debug)]
pub enum CmdInfo {
    StatusModified(bool),
    RemoteUrl { domain: String, path: String },
    Branch(String),
    LastCommitSummary(String),
    LastCommitMessage(String),
    Project(Project),
    Members(Vec<Member>),
    MergeRequest(MergeRequestResponse),
    MergeRequestsList(Vec<MergeRequestResponse>),
    OutgoingCommits(String),
    Ignore,
    Exit,
}

/// Adapts lower level I/O HTTP/Shell outputs to a common Response.
#[derive(Clone, Debug, Builder)]
pub struct Response {
    #[builder(default)]
    pub status: i32,
    #[builder(default)]
    pub body: String,
    /// Optional headers. Mostly used by HTTP downstream HTTP responses
    #[builder(setter(into, strip_option), default)]
    pub(crate) headers: Option<HashMap<String, String>>,
    #[builder(default = "parse_link_headers")]
    link_header_processor: fn(&str) -> PageHeader,
}

impl Response {
    pub fn builder() -> ResponseBuilder {
        ResponseBuilder::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseField {
    Body,
    Status,
    Headers,
}

impl Response {
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .as_ref()
            .and_then(|h| h.get(key))
            .map(|s| s.as_str())
    }

    pub fn get_page_headers(&self) -> Option<PageHeader> {
        if let Some(headers) = &self.headers {
            match headers.get(LINK_HEADER) {
                Some(link) => return Some((self.link_header_processor)(link)),
                None => return None,
            }
        }
        None
    }

    // Defaults:
    // https://docs.gitlab.com/ee/user/gitlab_com/index.html#gitlabcom-specific-rate-limits
    // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#primary-rate-limit-for-authenticated-users

    // Github 5000 requests per hour for authenticated users
    // Gitlab 2000 requests per minute for authenticated users
    // Most limiting Github 5000/60 = 83.33 requests per minute

    pub fn get_ratelimit_headers(&self) -> Option<RateLimitHeader> {
        let mut ratelimit_header = RateLimitHeader::default();

        // process remote headers and patch the defaults accordingly
        if let Some(headers) = &self.headers {
            if let Some(github_remaining) = headers.get(GITHUB_RATELIMIT_REMAINING) {
                ratelimit_header.remaining = github_remaining.parse::<u32>().unwrap_or(0);
                if let Some(github_reset) = headers.get(GITHUB_RATELIMIT_RESET) {
                    ratelimit_header.reset = Seconds::new(github_reset.parse::<u64>().unwrap_or(0));
                }
                return Some(ratelimit_header);
            }
            if let Some(gitlab_remaining) = headers.get(GITLAB_RATELIMIT_REMAINING) {
                ratelimit_header.remaining = gitlab_remaining.parse::<u32>().unwrap_or(0);
                if let Some(gitlab_reset) = headers.get(GITLAB_RATELIMIT_RESET) {
                    ratelimit_header.reset = Seconds::new(gitlab_reset.parse::<u64>().unwrap_or(0));
                }
                return Some(ratelimit_header);
            }
        }
        None
    }

    pub fn get_etag(&self) -> Option<&str> {
        self.header("etag")
    }
}

const NEXT: &str = "next";
const LAST: &str = "last";
pub const LINK_HEADER: &str = "link";

pub fn parse_link_headers(link: &str) -> PageHeader {
    lazy_static! {
        static ref RE_URL: Regex = Regex::new(r#"<([^>]+)>;\s*rel="([^"]+)""#).unwrap();
        static ref RE_PAGE_NUMBER: Regex = Regex::new(r"[^(per_)]page=(\d+)").unwrap();
    }
    let mut page_header = PageHeader::new();
    for cap in RE_URL.captures_iter(link) {
        if cap.len() > 2 && &cap[2] == NEXT {
            let url = cap[1].to_string();
            for page_cap in RE_PAGE_NUMBER.captures_iter(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_next_page(page);
                    continue;
                }
            }
        }
        // TODO pull code out - return a page and its type next or last.
        if cap.len() > 2 && &cap[2] == LAST {
            let url = cap[1].to_string();
            for page_cap in RE_PAGE_NUMBER.captures_iter(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_last_page(page);
                }
            }
        }
    }
    page_header
}

#[derive(Default)]
pub struct PageHeader {
    pub next: Option<Page>,
    pub last: Option<Page>,
}

impl PageHeader {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_next_page(&mut self, page: Page) {
        self.next = Some(page);
    }

    pub fn set_last_page(&mut self, page: Page) {
        self.last = Some(page);
    }
}

#[derive(Debug, PartialEq)]
pub struct Page {
    pub url: String,
    pub number: u32,
}

impl Page {
    pub fn new(url: &str, number: u32) -> Self {
        Page {
            url: url.to_string(),
            number,
        }
    }
}

// https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit

pub const GITHUB_RATELIMIT_REMAINING: &str = "x-ratelimit-remaining";
pub const GITHUB_RATELIMIT_RESET: &str = "x-ratelimit-reset";

// https://docs.gitlab.com/ee/administration/settings/user_and_ip_rate_limits.html

// Internal processing is all in lowercase
// Docs: RateLimit-Remaining
pub const GITLAB_RATELIMIT_REMAINING: &str = "ratelimit-remaining";
// Docs: RateLimit-Reset
pub const GITLAB_RATELIMIT_RESET: &str = "ratelimit-reset";

/// Unifies the different ratelimit headers available from the different remotes.
/// Github API ratelimit headers:
/// remaining: x-ratelimit-remaining
/// reset: x-ratelimit-reset
/// Gitlab API ratelimit headers:
/// remaining: RateLimit-Remaining
/// reset: RateLimit-Reset
#[derive(Clone, Debug, Default)]
pub struct RateLimitHeader {
    // The number of requests remaining in the current rate limit window.
    pub remaining: u32,
    // Unix time-formatted time when the request quota is reset.
    pub reset: Seconds,
}

impl RateLimitHeader {
    pub fn new(remaining: u32, reset: Seconds) -> Self {
        RateLimitHeader { remaining, reset }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_rate_limit_headers_github() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("x-ratelimit-remaining".to_string(), "30".to_string());
        headers.insert("x-ratelimit-reset".to_string(), "1658602270".to_string());
        let response = Response::builder()
            .body(body.to_string())
            .headers(headers)
            .build()
            .unwrap();
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining.clone());
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
    }

    #[test]
    fn test_get_rate_limit_headers_gitlab() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("ratelimit-remaining".to_string(), "30".to_string());
        headers.insert("ratelimit-reset".to_string(), "1658602270".to_string());
        let response = Response::builder()
            .body(body.to_string())
            .headers(headers)
            .build()
            .unwrap();
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining);
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
    }

    #[test]
    fn test_get_rate_limit_headers_camelcase_gitlab() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("RateLimit-remaining".to_string(), "30".to_string());
        headers.insert("rateLimit-reset".to_string(), "1658602270".to_string());
        let response = Response::builder()
            .body(body.to_string())
            .headers(headers)
            .build()
            .unwrap();
        let ratelimit_headers = response.get_ratelimit_headers();
        assert!(ratelimit_headers.is_none());
    }

    #[test]
    fn test_link_header_has_next_and_last_page() {
        let link = r#"<https://api.github.com/search/code?q=addClass+user%3Amozilla&page=2>; rel="next", <https://api.github.com/search/code?q=addClass+user%3Amozilla&page=34>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(
            "https://api.github.com/search/code?q=addClass+user%3Amozilla&page=2",
            page_headers.next.as_ref().unwrap().url
        );
        assert_eq!(2, page_headers.next.unwrap().number);
        assert_eq!(
            "https://api.github.com/search/code?q=addClass+user%3Amozilla&page=34",
            page_headers.last.as_ref().unwrap().url
        );
        assert_eq!(34, page_headers.last.unwrap().number);
    }

    #[test]
    fn test_link_header_has_no_next_page() {
        let link = r#"<http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel="first", <http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(None, page_headers.next);
    }

    #[test]
    fn test_with_per_page() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=2&per_page=20&sort=desc>; rel="next", <https://gitlab.disney.com/api/v4/projects/15/pipelines?id=15&order_by=id&page=1&per_page=20&sort=desc>; rel="first", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=91&per_page=20&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(2, page_headers.next.unwrap().number);
    }
}

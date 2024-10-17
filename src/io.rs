use crate::{
    api_defaults,
    cmds::{
        merge_request::MergeRequestResponse,
        project::{Member, Project},
    },
    http::{self, Headers, Request},
    log_info,
    remote::RemoteURL,
    time::{self, Milliseconds, Seconds},
    Result,
};
use regex::Regex;
use serde::Serialize;
use std::{
    ffi::OsStr,
    fmt::{self, Display, Formatter},
    rc::Rc,
    thread,
};

use rand::Rng;

/// A trait that handles the execution of processes with a finite lifetime. For
/// example, it can be an in-memory process for testing or a shell command doing
/// I/O. It handles all processes that do not conform with the HTTP protocol.
/// For that, check the `HttpRunner`
pub trait TaskRunner {
    type Response;
    fn run<T>(&self, cmd: T) -> Result<Self::Response>
    where
        T: IntoIterator,
        T::Item: AsRef<OsStr>;
}

/// A trait for the HTTP protocol. Implementors need to conform with the HTTP
/// constraints and requirements. Implementors accept a `Request` that wraps
/// headers, payloads and HTTP methods. Clients can potentially do HTTP calls
/// against a remote server or mock the responses for testing purposes.
pub trait HttpRunner {
    type Response;
    fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response>;
    /// Return the number of API MAX PAGES allowed for the given Request.
    fn api_max_pages<T: Serialize>(&self, cmd: &Request<T>) -> u32;
    /// Milliseconds to wait before executing the next request
    fn throttle(&self, milliseconds: Milliseconds) {
        thread::sleep(std::time::Duration::from_millis(*milliseconds));
    }
    /// Random wait time between the given range before submitting the next HTTP
    /// request. The wait time is in milliseconds. The range is inclusive.
    fn throttle_range(&self, min: Milliseconds, max: Milliseconds) {
        let mut rng = rand::thread_rng();
        let wait_time = rng.gen_range(*min..=*max);
        log_info!("Sleeping for {} milliseconds", wait_time);
        thread::sleep(std::time::Duration::from_millis(wait_time));
    }
}

#[derive(Clone, Debug)]
pub enum CmdInfo {
    StatusModified(bool),
    RemoteUrl(RemoteURL),
    Branch(String),
    CommitSummary(String),
    CommitMessage(String),
    Project(Project),
    Members(Vec<Member>),
    MergeRequest(MergeRequestResponse),
    MergeRequestsList(Vec<MergeRequestResponse>),
    OutgoingCommits(String),
    Ignore,
    Exit,
}

#[derive(Clone, Debug, Builder)]
pub struct ShellResponse {
    #[builder(default)]
    pub status: i32,
    #[builder(default)]
    pub body: String,
}

impl ShellResponse {
    pub fn builder() -> ShellResponseBuilder {
        ShellResponseBuilder::default()
    }
}

/// Adapts lower level I/O HTTP/Shell outputs to a common Response.
#[derive(Clone, Debug, Builder)]
pub struct HttpResponse {
    #[builder(default)]
    pub status: i32,
    #[builder(default)]
    pub body: String,
    /// Optional headers. Mostly used by HTTP downstream HTTP responses
    #[builder(setter(into, strip_option), default)]
    pub headers: Option<Headers>,
    #[builder(setter(into), default)]
    pub flow_control_headers: FlowControlHeaders,
}

impl HttpResponse {
    pub fn builder() -> HttpResponseBuilder {
        HttpResponseBuilder::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseField {
    Body,
    Status,
    Headers,
}

impl HttpResponse {
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .as_ref()
            .and_then(|h| h.get(key))
            .map(|s| s.as_str())
    }

    pub fn get_page_headers(&self) -> Rc<Option<PageHeader>> {
        self.flow_control_headers.get_page_header()
    }

    pub fn get_ratelimit_headers(&self) -> Rc<Option<RateLimitHeader>> {
        self.flow_control_headers.get_rate_limit_header()
    }

    pub fn get_etag(&self) -> Option<&str> {
        self.header("etag")
    }

    pub fn is_ok(&self, method: &http::Method) -> bool {
        match method {
            http::Method::HEAD => self.status == 200,
            http::Method::GET => self.status == 200,
            http::Method::POST => {
                self.status >= 200 && self.status < 300 || self.status == 409 || self.status == 422
            }
            http::Method::PATCH | http::Method::PUT => self.status >= 200 && self.status < 300,
        }
    }

    pub fn update_rate_limit_headers(&mut self, headers: RateLimitHeader) {
        self.flow_control_headers.rate_limit_header = Rc::new(Some(headers));
    }
}

const NEXT: &str = "next";
const LAST: &str = "last";
pub const LINK_HEADER: &str = "link";

fn parse_link_headers(link: &str) -> PageHeader {
    lazy_static! {
        static ref RE_URL: Regex = Regex::new(r#"<([^>]+)>;\s*rel="([^"]+)""#).unwrap();
        static ref RE_PAGE_NUMBER: Regex = Regex::new(r"[^(per_)]page=(\d+)").unwrap();
        static ref RE_PER_PAGE: Regex = Regex::new(r"per_page=(\d+)").unwrap();
    }
    let mut page_header = PageHeader::new();
    'links: for cap in RE_URL.captures_iter(link) {
        if cap.len() > 2 && &cap[2] == NEXT {
            // Capture per_page in next page if available to avoid re-computing
            // this section in next matches like `first` and `last`
            if let Some(per_page) = RE_PER_PAGE.captures(&cap[1]) {
                if per_page.len() > 1 {
                    let per_page = per_page[1].to_string();
                    let per_page: u32 = per_page.parse().unwrap_or(api_defaults::DEFAULT_PER_PAGE);
                    page_header.per_page = per_page;
                }
            } else {
                page_header.per_page = api_defaults::DEFAULT_PER_PAGE;
            };
            let url = cap[1].to_string();
            if let Some(page_cap) = RE_PAGE_NUMBER.captures(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_next_page(page);
                    continue 'links;
                }
            }
        }
        // TODO pull code out - return a page and its type next or last.
        if cap.len() > 2 && &cap[2] == LAST {
            let url = cap[1].to_string();
            if let Some(page_cap) = RE_PAGE_NUMBER.captures(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_last_page(page);
                }
            }
        }
    }
    if page_header.per_page == 0 {
        page_header.per_page = api_defaults::DEFAULT_PER_PAGE;
    }
    page_header
}

#[derive(Clone, Debug, Default)]
pub struct PageHeader {
    pub next: Option<Page>,
    pub last: Option<Page>,
    pub per_page: u32,
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

    pub fn next_page(&self) -> Option<&Page> {
        self.next.as_ref()
    }

    pub fn last_page(&self) -> Option<&Page> {
        self.last.as_ref()
    }
}

pub fn parse_page_headers(headers: Option<&Headers>) -> Option<PageHeader> {
    if let Some(headers) = headers {
        match headers.get(LINK_HEADER) {
            Some(link) => return Some(parse_link_headers(link)),
            None => return None,
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
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

    pub fn url(&self) -> &str {
        &self.url
    }
}

// https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit

pub const GITHUB_RATELIMIT_REMAINING: &str = "x-ratelimit-remaining";
pub const GITHUB_RATELIMIT_RESET: &str = "x-ratelimit-reset";

// Time to wait before retrying the next request - standard common header
// Gitlab Docs: Retry-After
pub const RETRY_AFTER: &str = "retry-after";

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
#[derive(Clone, Copy, Debug, Default)]
pub struct RateLimitHeader {
    // The number of requests remaining in the current rate limit window.
    pub remaining: u32,
    // Unix time-formatted time when the request quota is reset.
    pub reset: Seconds,
    // Time to wait before retrying the next request
    pub retry_after: Seconds,
}

impl RateLimitHeader {
    pub fn new(remaining: u32, reset: Seconds, retry_after: Seconds) -> Self {
        RateLimitHeader {
            remaining,
            reset,
            retry_after,
        }
    }
}

// Defaults:
// https://docs.gitlab.com/ee/user/gitlab_com/index.html#gitlabcom-specific-rate-limits
// https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#primary-rate-limit-for-authenticated-users

// Github 5000 requests per hour for authenticated users
// Gitlab 2000 requests per minute for authenticated users
// Most limiting Github 5000/60 = 83.33 requests per minute

pub fn parse_ratelimit_headers(headers: Option<&Headers>) -> Option<RateLimitHeader> {
    let mut ratelimit_header = RateLimitHeader::default();

    // process remote headers and patch the defaults accordingly
    if let Some(headers) = headers {
        if let Some(retry_after) = headers.get(RETRY_AFTER) {
            ratelimit_header.retry_after = Seconds::new(retry_after.parse::<u64>().unwrap_or(0));
        }
        if let Some(github_remaining) = headers.get(GITHUB_RATELIMIT_REMAINING) {
            ratelimit_header.remaining = github_remaining.parse::<u32>().unwrap_or(0);
            if let Some(github_reset) = headers.get(GITHUB_RATELIMIT_RESET) {
                ratelimit_header.reset = Seconds::new(github_reset.parse::<u64>().unwrap_or(0));
            }
            log_info!("Header {}", ratelimit_header);
            return Some(ratelimit_header);
        }
        if let Some(gitlab_remaining) = headers.get(GITLAB_RATELIMIT_REMAINING) {
            ratelimit_header.remaining = gitlab_remaining.parse::<u32>().unwrap_or(0);
            if let Some(gitlab_reset) = headers.get(GITLAB_RATELIMIT_RESET) {
                ratelimit_header.reset = Seconds::new(gitlab_reset.parse::<u64>().unwrap_or(0));
            }
            log_info!("Header {}", ratelimit_header);
            return Some(ratelimit_header);
        }
    }
    None
}

impl Display for RateLimitHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let reset = time::epoch_to_minutes_relative(self.reset);
        write!(
            f,
            "RateLimitHeader: remaining: {}, reset in: {} minutes",
            self.remaining, reset
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct FlowControlHeaders {
    page_header: Rc<Option<PageHeader>>,
    rate_limit_header: Rc<Option<RateLimitHeader>>,
}

impl FlowControlHeaders {
    pub fn new(
        page_header: Rc<Option<PageHeader>>,
        rate_limit_header: Rc<Option<RateLimitHeader>>,
    ) -> Self {
        FlowControlHeaders {
            page_header,
            rate_limit_header,
        }
    }

    pub fn get_page_header(&self) -> Rc<Option<PageHeader>> {
        self.page_header.clone()
    }

    pub fn get_rate_limit_header(&self) -> Rc<Option<RateLimitHeader>> {
        self.rate_limit_header.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_rate_limit_headers_github() {
        let body = "responsebody";
        let mut headers = Headers::new();
        headers.set("x-ratelimit-remaining".to_string(), "30".to_string());
        headers.set("x-ratelimit-reset".to_string(), "1658602270".to_string());
        headers.set("retry-after".to_string(), "60".to_string());
        let rate_limit_header = parse_ratelimit_headers(Some(&headers)).unwrap();
        let flow_control_headers =
            FlowControlHeaders::new(Rc::new(None), Rc::new(Some(rate_limit_header)));
        let response = HttpResponse::builder()
            .body(body.to_string())
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining.clone());
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
        assert_eq!(Seconds::new(60), ratelimit_headers.retry_after);
    }

    #[test]
    fn test_get_rate_limit_headers_gitlab() {
        let body = "responsebody";
        let mut headers = Headers::new();
        headers.set("ratelimit-remaining".to_string(), "30".to_string());
        headers.set("ratelimit-reset".to_string(), "1658602270".to_string());
        headers.set("retry-after".to_string(), "60".to_string());
        let rate_limit_header = parse_ratelimit_headers(Some(&headers)).unwrap();
        let flow_control_headers =
            FlowControlHeaders::new(Rc::new(None), Rc::new(Some(rate_limit_header)));
        let response = HttpResponse::builder()
            .body(body.to_string())
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining);
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
        assert_eq!(Seconds::new(60), ratelimit_headers.retry_after);
    }

    #[test]
    fn test_get_rate_limit_headers_camelcase_gitlab() {
        let body = "responsebody";
        let mut headers = Headers::new();
        headers.set("RateLimit-remaining".to_string(), "30".to_string());
        headers.set("rateLimit-reset".to_string(), "1658602270".to_string());
        headers.set("Retry-After".to_string(), "60".to_string());
        let rate_limit_header = parse_ratelimit_headers(Some(&headers));
        let flow_control_headers =
            FlowControlHeaders::new(Rc::new(None), Rc::new(rate_limit_header));
        let response = HttpResponse::builder()
            .body(body.to_string())
            .headers(headers)
            .flow_control_headers(flow_control_headers)
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
    fn test_link_header_has_first_next_and_last() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=2&per_page=20&sort=desc>; rel="next", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=1&per_page=20&sort=desc>; rel="first", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=91&per_page=20&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(2, page_headers.next.unwrap().number);
    }

    #[test]
    fn test_response_ok_status_get_request_200() {
        assert!(HttpResponse::builder()
            .status(200)
            .build()
            .unwrap()
            .is_ok(&http::Method::GET));
    }

    #[test]
    fn test_response_not_ok_if_get_request_400s() {
        let not_ok_status = 400..=499;
        for status in not_ok_status {
            let response = HttpResponse::builder().status(status).build().unwrap();
            assert!(!response.is_ok(&http::Method::GET));
        }
    }

    #[test]
    fn test_response_ok_status_post_request_201() {
        assert!(HttpResponse::builder()
            .status(201)
            .build()
            .unwrap()
            .is_ok(&http::Method::POST));
    }

    #[test]
    fn test_response_ok_if_post_request_409_422() {
        // special case handled by the caller (merge_request)
        let not_ok_status = [409, 422];
        for status in not_ok_status.iter() {
            let response = HttpResponse::builder().status(*status).build().unwrap();
            assert!(response.is_ok(&http::Method::POST));
        }
    }

    #[test]
    fn test_response_not_ok_if_500s_any_case() {
        let methods = [
            http::Method::GET,
            http::Method::POST,
            http::Method::PATCH,
            http::Method::PUT,
        ];
        let not_ok_status = 500..=599;
        for status in not_ok_status {
            for method in methods.iter() {
                let response = HttpResponse::builder().status(status).build().unwrap();
                assert!(!response.is_ok(method));
            }
        }
    }

    #[test]
    fn test_link_headers_get_per_page_multiple_pages() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=2&per_page=20&sort=desc>; rel="next", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=1&per_page=20&sort=desc>; rel="first", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=91&per_page=20&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(2, page_headers.next.unwrap().number);
        assert_eq!(20, page_headers.per_page);
    }

    #[test]
    fn test_link_headers_get_per_page_not_available_use_default() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=2&sort=desc>; rel="next", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=1&sort=desc>; rel="first", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=91&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(2, page_headers.next.unwrap().number);
        assert_eq!(api_defaults::DEFAULT_PER_PAGE, page_headers.per_page);
    }

    #[test]
    fn test_link_headers_get_per_page_with_no_next_use_default() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=1&sort=desc>; rel="first", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=91&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(None, page_headers.next);
        assert_eq!(api_defaults::DEFAULT_PER_PAGE, page_headers.per_page);
    }

    #[test]
    fn test_link_headers_get_per_page_available_in_last_only_use_default() {
        let link = r#"<https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&page=2&sort=desc>; rel="next", <https://gitlab-web/api/v4/projects/15/pipelines?id=15&order_by=id&per_page=20&page=91&sort=desc>; rel="last""#;
        let page_headers = parse_link_headers(link);
        assert_eq!(91, page_headers.last.unwrap().number);
        assert_eq!(2, page_headers.next.unwrap().number);
        assert_eq!(api_defaults::DEFAULT_PER_PAGE, page_headers.per_page);
    }
}

use crate::{
    http::Request,
    remote::{Member, MergeRequestResponse, Project},
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
}

#[derive(Debug)]
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
#[derive(Clone, Debug)]
pub struct Response {
    pub status: i32,
    pub body: String,
    /// Optional headers. Mostly used by HTTP downstream HTTP responses
    pub(crate) headers: Option<HashMap<String, String>>,
    link_header_processor: fn(&str) -> PageHeader,
}

impl Response {
    pub fn new() -> Self {
        Self {
            status: 0,
            body: String::new(),
            headers: None,
            link_header_processor: parse_link_headers,
        }
    }

    pub fn with_header_processor(mut self, processor: fn(&str) -> PageHeader) -> Self {
        self.link_header_processor = processor;
        self
    }

    pub fn with_status(mut self, status: i32) -> Self {
        self.status = status;
        self
    }

    pub fn with_body(mut self, output: String) -> Self {
        self.body = output;
        self
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
    }

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

    pub fn get_etag(&self) -> Option<&str> {
        self.header("etag")
    }

    pub fn status(&self) -> i32 {
        self.status
    }
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
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

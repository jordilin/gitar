use crate::api_traits::ApiOperation;
use crate::error;
use crate::http::Method::GET;
use crate::http::{self, Body, Headers};
use crate::remote::{query, MergeRequestListBodyArgs};
use crate::Result;
use crate::{
    api_traits::MergeRequest,
    io::{HttpRunner, Response},
    remote::{MergeRequestBodyArgs, MergeRequestResponse},
};

use crate::json_loads;

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> MergeRequest for Gitlab<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body = Body::new();
        body.add("source_branch", args.source_branch);
        body.add("target_branch", args.target_branch);
        body.add("title", args.title);
        body.add("assignee_id", args.assignee_id);
        body.add("description", args.description);
        body.add("remove_source_branch", args.remove_source_branch);
        let url = format!("{}/merge_requests", self.rest_api_basepath());
        let response = query::gitlab_merge_request_response(
            &self.runner,
            &url,
            Some(body),
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        )?;
        // if status code is 409, it means that the merge request already
        // exists. We already pushed the branch, just return the merge request
        // as if it was created.
        if response.status == 409 {
            // {\"message\":[\"Another open merge request already exists for
            // this source branch: !60\"]}"
            let merge_request_json: serde_json::Value = serde_json::from_str(&response.body)?;
            let merge_request_iid = merge_request_json["message"][0]
                .as_str()
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_matches('!');
            let merge_request_url = format!(
                "https://{}/{}/-/merge_requests/{}",
                self.domain, self.path, merge_request_iid
            );
            return Ok(MergeRequestResponse::builder()
                .id(merge_request_iid.parse().unwrap())
                .web_url(merge_request_url)
                .build()
                .unwrap());
        }
        if response.status != 201 {
            return Err(error::gen(format!(
                "Failed to open merge request: {}",
                response.body
            )));
        }
        let merge_request_json = json_loads(&response.body)?;

        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .build()
            .unwrap())
    }

    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>> {
        let url = format!(
            "{}/merge_requests?state={}",
            self.rest_api_basepath(),
            args.state
        );
        query::gitlab_list_merge_requests(
            &self.runner,
            &url,
            args.list_args,
            self.headers(),
            None,
            ApiOperation::MergeRequest,
        )
    }

    fn merge(&self, id: i64) -> Result<MergeRequestResponse> {
        // PUT /projects/:id/merge_requests/:merge_request_iid/merge
        let url = format!("{}/merge_requests/{}/merge", self.rest_api_basepath(), id);
        query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )
    }

    fn get(&self, id: i64) -> Result<MergeRequestResponse> {
        // GET /projects/:id/merge_requests/:merge_request_iid
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            GET,
            ApiOperation::MergeRequest,
        )
    }

    fn close(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        let mut body = Body::new();
        body.add("state_event".to_string(), "close".to_string());
        query::gitlab_merge_request::<_, String>(
            &self.runner,
            &url,
            Some(body),
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let state = args.state.to_string();
        let url = format!(
            "{}/merge_requests?state={}&page=1",
            self.rest_api_basepath(),
            state
        );
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }
}

pub struct GitlabMergeRequestFields {
    id: i64,
    web_url: String,
    source_branch: String,
    author: String,
    updated_at: String,
    created_at: String,
}

impl From<&serde_json::Value> for GitlabMergeRequestFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMergeRequestFields {
            id: data["iid"].as_i64().unwrap(),
            web_url: data["web_url"].as_str().unwrap().to_string(),
            source_branch: data["source_branch"].as_str().unwrap().to_string(),
            author: data["author"]["username"].as_str().unwrap().to_string(),
            updated_at: data["updated_at"].as_str().unwrap().to_string(),
            created_at: data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabMergeRequestFields> for MergeRequestResponse {
    fn from(fields: GitlabMergeRequestFields) -> Self {
        MergeRequestResponse::builder()
            .id(fields.id)
            .web_url(fields.web_url)
            .source_branch(fields.source_branch)
            .author(fields.author)
            .updated_at(fields.updated_at)
            .created_at(fields.created_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use crate::remote::{ListBodyArgs, MergeRequestState};
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use super::*;

    #[test]
    fn test_list_merge_request_with_from_page() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(2)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2",
            *client.url(),
        );
    }

    #[test]
    fn test_open_merge_request() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(201)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());

        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_error() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);
        assert!(gitlab.open(mr_args).is_err());
    }
    #[test]
    fn test_merge_request_already_exists_status_code_409_conflict() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(409)
            .body(get_contract(
                ContractType::Gitlab,
                "merge_request_conflict.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);

        assert!(gitlab.open(mr_args).is_ok());
    }
    #[test]
    fn test_gitlab_merge_request_num_pages() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_link_header_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_response_error_is_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_last_header_in_link() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(None, gitlab.num_pages(body_args).unwrap());
    }
}

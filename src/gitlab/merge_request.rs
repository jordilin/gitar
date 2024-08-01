use crate::api_traits::{ApiOperation, CommentMergeRequest, NumberDeltaErr, RemoteProject};
use crate::cli::browse::BrowseOptions;
use crate::cmds::merge_request::{
    Comment, CommentMergeRequestBodyArgs, CommentMergeRequestListBodyArgs,
};
use crate::error::{self, GRError};
use crate::http::Method::GET;
use crate::http::{self, Body, Headers};
use crate::io::CmdInfo;
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
        // if target repo provided, add target_project_id in the payload
        if !args.target_repo.is_empty() {
            match self.get_project_data(None, Some(&args.target_repo)) {
                Ok(CmdInfo::Project(project)) => {
                    body.add("target_project_id", project.id.to_string());
                }
                Ok(_) => {
                    // Application error - any other CmdInfo variant is unexpected
                    return Err(GRError::ApplicationError(
                        "Failed to get target project data".to_string(),
                    )
                    .into());
                }
                Err(e) => {
                    return Err(error::gen(format!(
                        "Could not get target project data for {} with error {}",
                        args.target_repo, e
                    )))
                }
            }
        }
        let url = format!("{}/merge_requests", self.rest_api_basepath());
        let response = query::gitlab_merge_request_response(
            &self.runner,
            &url,
            Some(&body),
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
            if args.amend {
                let url = format!(
                    "{}/merge_requests/{}",
                    self.rest_api_basepath(),
                    merge_request_iid
                );
                query::gitlab_merge_request_response(
                    &self.runner,
                    &url,
                    Some(&body),
                    self.headers(),
                    http::Method::PUT,
                    ApiOperation::MergeRequest,
                )?;
            }
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
        let url = self.list_merge_request_url(&args, false);
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
        body.add("state_event", "close");
        query::gitlab_merge_request::<_, &str>(
            &self.runner,
            &url,
            Some(&body),
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let url = self.list_merge_request_url(&args, true);
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }

    fn num_resources(&self, args: MergeRequestListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        let url = self.list_merge_request_url(&args, true);
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_resources(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }

    fn approve(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!("{}/merge_requests/{}/approve", self.rest_api_basepath(), id);
        let result = query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        );
        // responses in approvals for Gitlab do not contain the merge request
        // URL, patch it in the response.
        if let Ok(mut response) = result {
            response.web_url = self.get_url(BrowseOptions::MergeRequestId(id));
            return Ok(response);
        }
        result
    }
}

impl<R> Gitlab<R> {
    fn list_merge_request_url(&self, args: &MergeRequestListBodyArgs, num_pages: bool) -> String {
        let mut url = if let Some(assignee) = &args.assignee {
            format!(
                "{}?state={}&assignee_id={}",
                self.merge_requests_url, args.state, assignee.id
            )
        } else if let Some(reviewer) = &args.reviewer {
            format!(
                "{}?state={}&reviewer_id={}",
                self.merge_requests_url, args.state, reviewer.id
            )
        } else if let Some(author) = &args.author {
            format!(
                "{}?state={}&author_id={}",
                self.merge_requests_url, args.state, author.id
            )
        } else {
            format!(
                "{}/merge_requests?state={}",
                self.rest_api_basepath(),
                args.state
            )
        };
        if num_pages {
            url.push_str("&page=1");
        }
        url
    }

    fn resource_comments_metadata_url(&self, args: CommentMergeRequestListBodyArgs) -> String {
        let url = format!(
            "{}/merge_requests/{}/notes?page=1",
            self.rest_api_basepath(),
            args.id
        );
        url
    }
}

impl<R: HttpRunner<Response = Response>> CommentMergeRequest for Gitlab<R> {
    fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()> {
        let url = format!(
            "{}/merge_requests/{}/notes",
            self.rest_api_basepath(),
            args.id
        );
        let mut body = Body::new();
        body.add("body", args.comment);
        query::create_merge_request_comment(
            &self.runner,
            &url,
            Some(&body),
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        )?;
        Ok(())
    }

    fn list(&self, args: CommentMergeRequestListBodyArgs) -> Result<Vec<Comment>> {
        let url = format!(
            "{}/merge_requests/{}/notes",
            self.rest_api_basepath(),
            args.id
        );

        query::gitlab_list_merge_request_comments(
            &self.runner,
            &url,
            args.list_args,
            self.headers(),
            None,
            ApiOperation::MergeRequest,
        )
    }

    fn num_pages(&self, args: CommentMergeRequestListBodyArgs) -> Result<Option<u32>> {
        let url = self.resource_comments_metadata_url(args);
        query::num_pages(
            &self.runner,
            &url,
            self.headers(),
            ApiOperation::MergeRequest,
        )
    }

    fn num_resources(
        &self,
        args: CommentMergeRequestListBodyArgs,
    ) -> Result<Option<NumberDeltaErr>> {
        let url = self.resource_comments_metadata_url(args);
        query::num_resources(
            &self.runner,
            &url,
            self.headers(),
            ApiOperation::MergeRequest,
        )
    }
}

pub struct GitlabMergeRequestFields {
    fields: MergeRequestResponse,
}

impl From<&serde_json::Value> for GitlabMergeRequestFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMergeRequestFields {
            fields: MergeRequestResponse::builder()
                .id(data["iid"].as_i64().unwrap_or_default())
                .web_url(data["web_url"].as_str().unwrap_or_default().to_string())
                .source_branch(
                    data["source_branch"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .sha(
                    data["merge_commit_sha"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .author(
                    data["author"]["username"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .updated_at(data["updated_at"].as_str().unwrap_or_default().to_string())
                .created_at(data["created_at"].as_str().unwrap_or_default().to_string())
                .title(data["title"].as_str().unwrap_or_default().to_string())
                .description(data["description"].as_str().unwrap_or_default().to_string())
                // If merge request is not merged, merged_at is an empty string.
                .merged_at(data["merged_at"].as_str().unwrap_or_default().to_string())
                // Documentation recommends gathering head_pipeline instead of
                // pipeline key.
                .pipeline_id(data["head_pipeline"]["id"].as_i64())
                .pipeline_url(
                    data["head_pipeline"]["web_url"]
                        .as_str()
                        .map(|s| s.to_string()),
                )
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabMergeRequestFields> for MergeRequestResponse {
    fn from(fields: GitlabMergeRequestFields) -> Self {
        fields.fields
    }
}

pub struct GitlabMergeRequestCommentFields {
    comment: Comment,
}

impl From<&serde_json::Value> for GitlabMergeRequestCommentFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMergeRequestCommentFields {
            comment: Comment::builder()
                .id(data["id"].as_i64().unwrap_or_default())
                .body(data["body"].as_str().unwrap_or_default().to_string())
                .author(
                    data["author"]["username"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .created_at(data["created_at"].as_str().unwrap_or_default().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabMergeRequestCommentFields> for Comment {
    fn from(fields: GitlabMergeRequestCommentFields) -> Self {
        fields.comment
    }
}

#[cfg(test)]
mod test {

    use crate::remote::{ListBodyArgs, Member, MergeRequestState};
    use crate::setup_client;
    use crate::test::utils::{
        default_gitlab, get_contract, BasePath, ClientType, ContractType, Domain, ResponseContracts,
    };

    use super::*;

    #[test]
    fn test_list_merge_request_with_from_page() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body(200, Some("[]"), None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(2)
                    .build()
                    .unwrap(),
            ))
            .assignee(None)
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2",
            *client.url(),
        );
    }

    #[test]
    fn test_list_all_merge_requests_assigned_for_current_user() {
        let contract = ResponseContracts::new(ContractType::Gitlab).add_body(200, Some("[]"), None);
        let (client, gitlab) = setup_client!(contract, default_gitlab(), dyn MergeRequest);
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(Some(
                Member::builder()
                    .name("tom".to_string())
                    .username("tsawyer".to_string())
                    .id(1234)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234",
            *client.url(),
        );
    }

    #[test]
    fn test_list_all_merge_requests_auth_user_is_reviewer() {
        let contract = ResponseContracts::new(ContractType::Gitlab).add_body(200, Some("[]"), None);
        let (client, gitlab) = setup_client!(contract, default_gitlab(), dyn MergeRequest);
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .reviewer(Some(
                Member::builder()
                    .name("tom".to_string())
                    .username("tsawyer".to_string())
                    .id(123)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&reviewer_id=123",
            *client.url(),
        );
    }

    #[test]
    fn test_list_all_merge_requests_auth_user_is_the_author() {
        let contract = ResponseContracts::new(ContractType::Gitlab).add_body(200, Some("[]"), None);
        let (client, gitlab) = setup_client!(contract, default_gitlab(), dyn MergeRequest);
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .author(Some(
                Member::builder()
                    .name("tom".to_string())
                    .username("tsawyer".to_string())
                    .id(192)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&author_id=192",
            *client.url(),
        );
    }

    #[test]
    fn test_open_merge_request() {
        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            201,
            "merge_request.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests",
            *client.url(),
        );
        let mut actual_method = client.http_method.borrow_mut();
        assert_eq!(http::Method::POST, actual_method.pop().unwrap());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_target_repo() {
        // current repo, targetting jordilin/gitar
        let client_type = ClientType::Gitlab(
            Domain("gitlab.com".to_string()),
            BasePath("jdoe/gitar".to_string()),
        );
        let responses = ResponseContracts::new(ContractType::Gitlab)
            .add_contract(201, "merge_request.json", None)
            .add_contract(200, "project.json", None);
        let (client, gitlab) = setup_client!(responses, client_type, dyn MergeRequest);
        let mr_args = MergeRequestBodyArgs::builder()
            .target_repo("jordilin/gitar".to_string())
            .build()
            .unwrap();
        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jdoe%2Fgitar/merge_requests",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_error() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(400, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        assert!(gitlab.open(mr_args).is_err());
    }

    #[test]
    fn test_merge_request_already_exists_status_code_409_conflict() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            409,
            "merge_request_conflict.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        assert!(gitlab.open(mr_args).is_ok());
    }

    #[test]
    fn test_amend_existing_merge_request() {
        let contracts = ResponseContracts::new(ContractType::Gitlab)
            .add_contract(200, "merge_request.json", None)
            .add_contract(409, "merge_request_conflict.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let mr_args = MergeRequestBodyArgs::builder().amend(true).build().unwrap();
        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33",
            *client.url()
        );
        let actual_method = client.http_method.borrow();
        assert_eq!(http::Method::PUT, actual_method[1]);
    }

    #[test]
    fn test_gitlab_merge_request_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_current_auth_user() {
        let link_header = "<https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=1>; rel=\"next\", <https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(Some(
                Member::builder()
                    .name("tom".to_string())
                    .username("tsawyer".to_string())
                    .id(1234)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_link_header_error() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(200, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(None)
            .build()
            .unwrap();
        assert_eq!(Some(1), gitlab.num_pages(body_args).unwrap());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_response_error_is_error() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(400, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_last_header_in_link() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee(None)
            .build()
            .unwrap();
        assert_eq!(None, gitlab.num_pages(body_args).unwrap());
    }

    #[test]
    fn test_gitlab_create_merge_request_comment_ok() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(201, None, None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CommentMergeRequest);
        let comment_args = CommentMergeRequestBodyArgs::builder()
            .id(1456)
            .comment("LGTM, ship it".to_string())
            .build()
            .unwrap();
        gitlab.create(comment_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/1456/notes",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_gitlab_create_merge_request_comment_error() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(400, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn CommentMergeRequest);
        let comment_args = CommentMergeRequestBodyArgs::builder()
            .id(1456)
            .comment("LGTM, ship it".to_string())
            .build()
            .unwrap();
        assert!(gitlab.create(comment_args).is_err());
    }

    #[test]
    fn test_get_gitlab_merge_request_details() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "merge_request.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let merge_request_id = 123456;
        gitlab.get(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123456",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_merge_merge_request() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "merge_request.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let merge_request_id = 33;
        gitlab.merge(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33/merge",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_close_merge_request() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "merge_request.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let merge_request_id = 33;
        gitlab.close(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33",
            *client.url()
        );
        let mut actual_method = client.http_method.borrow_mut();
        assert_eq!(http::Method::PUT, actual_method.pop().unwrap());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_approve_merge_request_ok() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "approve_merge_request.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn MergeRequest);
        let merge_request_id = 33;
        let result = gitlab.approve(merge_request_id);
        match result {
            Ok(response) => {
                assert_eq!(
                    "https://gitlab.com/jordilin/gitlapi/-/merge_requests/33",
                    response.web_url
                );
            }
            Err(e) => {
                panic!(
                    "Expected Ok merge request approval but got: {:?} instead",
                    e
                );
            }
        }
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33/approve",
            *client.url()
        );
        let mut actual_method = client.http_method.borrow_mut();
        assert_eq!(http::Method::POST, actual_method.pop().unwrap());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_list_merge_request_comments() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body(
            200,
            Some(format!(
                "[{}]",
                get_contract(ContractType::Gitlab, "comment.json")
            )),
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CommentMergeRequest);
        let args = CommentMergeRequestListBodyArgs::builder()
            .id(123)
            .list_args(None)
            .build()
            .unwrap();
        let comments = gitlab.list(args).unwrap();
        assert_eq!(1, comments.len());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123/notes",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_merge_request_comments_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123/notes?page=1>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123/notes?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CommentMergeRequest);
        let args = CommentMergeRequestListBodyArgs::builder()
            .id(123)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123/notes?page=1",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }
}

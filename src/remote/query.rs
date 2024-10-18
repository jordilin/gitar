use std::borrow::Borrow;
use std::iter::Iterator;
use std::sync::Arc;

use serde::Serialize;

use crate::api_traits::Timestamp;
use crate::display::DisplayBody;
use crate::http::throttle::{self, ThrottleStrategy};
use crate::{
    api_defaults,
    api_traits::{ApiOperation, NumberDeltaErr},
    display, error,
    http::{self, Body, Headers, Paginator, Request, Resource},
    io::{HttpResponse, HttpRunner},
    json_load_page, json_loads,
    remote::ListBodyArgs,
    time::sort_filter_by_date,
    Result,
};

fn get_remote_resource_headers<R: HttpRunner<Response = HttpResponse>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<HttpResponse> {
    send_request::<_, String>(
        runner,
        url,
        None,
        request_headers,
        http::Method::HEAD,
        api_operation,
    )
}

pub fn num_pages<R: HttpRunner<Response = HttpResponse>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<u32>> {
    let response = get_remote_resource_headers(runner, url, request_headers, api_operation)?;
    match response.get_page_headers().borrow() {
        Some(page_header) => {
            if let Some(last_page) = page_header.last_page() {
                return Ok(Some(last_page.number));
            }
            Ok(None)
        }
        // Github does not return page headers when there is only one page, so
        // we assume 1 page in this case.
        None => Ok(Some(1)),
    }
}

pub fn num_resources<R: HttpRunner<Response = HttpResponse>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<NumberDeltaErr>> {
    let response = get_remote_resource_headers(runner, url, request_headers, api_operation)?;
    match response.get_page_headers().borrow() {
        Some(page_header) => {
            // total resources per_page * total_pages
            if let Some(last_page) = page_header.last_page() {
                let count = last_page.number * page_header.per_page;
                return Ok(Some(NumberDeltaErr {
                    num: count,
                    delta: page_header.per_page,
                }));
            }
            Ok(None)
        }
        None => {
            // Github does not return page headers when there is only one page, so
            // we assume 1 page in this case.
            Ok(Some(NumberDeltaErr {
                num: 1,
                delta: api_defaults::DEFAULT_PER_PAGE,
            }))
        }
    }
}

pub fn query_error(url: &str, response: &HttpResponse) -> error::GRError {
    error::GRError::RemoteServerError(format!(
        "Failed to submit request to URL: {} with status code: {} and body: {}",
        url, response.status, response.body
    ))
}

pub fn send<R: HttpRunner<Response = HttpResponse>, D: Serialize, T>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
    mapper: impl Fn(&serde_json::Value) -> T,
    method: http::Method,
) -> Result<T> {
    let response = send_request(runner, url, body, request_headers, method, operation)?;
    let body = json_loads(&response.body)?;
    Ok(mapper(&body))
}

pub fn send_json<R: HttpRunner<Response = HttpResponse>, D: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
    method: http::Method,
) -> Result<serde_json::Value> {
    let response = send_request(runner, url, body, request_headers, method, operation)?;
    json_loads(&response.body)
}

pub fn send_raw<R: HttpRunner<Response = HttpResponse>, D: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
    method: http::Method,
) -> Result<HttpResponse> {
    send_request(runner, url, body, request_headers, method, operation)
}

pub fn get<R: HttpRunner<Response = HttpResponse>, D: Serialize, T>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
    mapper: impl Fn(&serde_json::Value) -> T,
) -> Result<T> {
    let response = send_request(
        runner,
        url,
        body,
        request_headers,
        http::Method::GET,
        operation,
    )?;
    let body = json_loads(&response.body)?;
    Ok(mapper(&body))
}

pub fn get_json<R: HttpRunner<Response = HttpResponse>, D: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
) -> Result<serde_json::Value> {
    let response = send_request(
        runner,
        url,
        body,
        request_headers,
        http::Method::GET,
        operation,
    )?;
    json_loads(&response.body)
}

pub fn get_raw<R: HttpRunner<Response = HttpResponse>, D: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<D>>,
    request_headers: Headers,
    operation: ApiOperation,
) -> Result<HttpResponse> {
    send_request(
        runner,
        url,
        body,
        request_headers,
        http::Method::GET,
        operation,
    )
}

fn send_request<R: HttpRunner<Response = HttpResponse>, T: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<&Body<T>>,
    request_headers: Headers,
    method: http::Method,
    operation: ApiOperation,
) -> Result<HttpResponse> {
    let mut request = if let Some(body) = body {
        http::Request::builder()
            .method(method.clone())
            .resource(Resource::new(url, Some(operation)))
            .body(body)
            .headers(request_headers)
            .build()
            .unwrap()
    } else {
        http::Request::builder()
            .method(method.clone())
            .resource(Resource::new(url, Some(operation)))
            .headers(request_headers)
            .build()
            .unwrap()
    };
    let response = runner.run(&mut request)?;
    // TODO: Might not be the right place as some APIs might still need to check
    // the response status code. See github merge request request reviewers when
    // a 422 is considered an error.
    if !response.is_ok(&method) {
        return Err(query_error(url, &response).into());
    }
    Ok(response)
}

pub fn paged<R, T>(
    runner: &Arc<R>,
    url: &str,
    list_args: Option<ListBodyArgs>,
    request_headers: Headers,
    iter_over_sub_array: Option<&str>,
    operation: ApiOperation,
    mapper: impl Fn(&serde_json::Value) -> T,
) -> Result<Vec<T>>
where
    R: HttpRunner<Response = HttpResponse>,
    T: Clone + Timestamp + Into<DisplayBody>,
{
    let request = build_list_request(url, &list_args, request_headers, operation);
    let mut throttle_time = None;
    let mut throttle_range = None;
    let mut backoff_max_retries = 0;
    let mut backoff_wait_time = 60;
    if let Some(list_args) = &list_args {
        throttle_time = list_args.throttle_time;
        throttle_range = list_args.throttle_range;
        backoff_max_retries = list_args.get_args.backoff_max_retries;
        backoff_wait_time = list_args.get_args.backoff_retry_after;
    }
    let throttle_strategy: Box<dyn ThrottleStrategy> = match throttle_time {
        Some(throttle_time) => Box::new(throttle::Fixed::new(throttle_time)),
        None => match throttle_range {
            Some((min, max)) => Box::new(throttle::Random::new(min, max)),
            None => Box::new(throttle::NoThrottle::new()),
        },
    };
    let paginator = Paginator::new(
        runner,
        request,
        url,
        backoff_max_retries,
        backoff_wait_time,
        throttle_strategy.as_ref(),
    );
    let all_data = paginator
        .map(|response| {
            let response = response?;
            if !response.is_ok(&http::Method::GET) {
                return Err(query_error(url, &response).into());
            }
            if iter_over_sub_array.is_some() {
                let body = json_loads(&response.body)?;
                let paged_data = body[iter_over_sub_array.unwrap()]
                    .as_array()
                    .ok_or_else(|| {
                        error::GRError::RemoteUnexpectedResponseContract(format!(
                            "Expected an array of {} but got: {}",
                            iter_over_sub_array.unwrap(),
                            response.body
                        ))
                    })?
                    .iter()
                    .fold(Vec::new(), |mut paged_data, data| {
                        paged_data.push(mapper(data));
                        paged_data
                    });
                if let Some(list_args) = &list_args {
                    if list_args.flush {
                        display::print(
                            &mut std::io::stdout(),
                            paged_data,
                            list_args.get_args.clone(),
                        )
                        .unwrap();
                        return Ok(Vec::new());
                    }
                }
                return Ok(paged_data);
            }
            let paged_data =
                json_load_page(&response.body)?
                    .iter()
                    .fold(Vec::new(), |mut paged_data, data| {
                        paged_data.push(mapper(data));
                        paged_data
                    });
            if let Some(list_args) = &list_args {
                if list_args.flush {
                    display::print(
                        &mut std::io::stdout(),
                        paged_data,
                        list_args.get_args.clone(),
                    )
                    .unwrap();
                    return Ok(Vec::new());
                }
            }
            Ok(paged_data)
        })
        .collect::<Result<Vec<Vec<T>>>>()
        .map(|paged_data| paged_data.into_iter().flatten().collect());
    match all_data {
        Ok(paged_data) => Ok(sort_filter_by_date(paged_data, list_args)?),
        Err(err) => Err(err),
    }
}

fn build_list_request<'a>(
    url: &str,
    list_args: &Option<ListBodyArgs>,
    request_headers: Headers,
    operation: ApiOperation,
) -> Request<'a, ()> {
    let mut request: http::Request<()> =
        http::Request::new(url, http::Method::GET).with_api_operation(operation);
    request.set_headers(request_headers);
    if let Some(list_args) = list_args {
        if let Some(from_page) = list_args.page {
            let url = if url.contains('?') {
                format!("{}&page={}", url, &from_page)
            } else {
                format!("{}?page={}", url, &from_page)
            };
            request.set_max_pages(list_args.max_pages.unwrap());
            request.set_url(&url);
        }
    }
    request
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use crate::{
        io::{FlowControlHeaders, Page, PageHeader},
        test::utils::MockRunner,
    };

    use super::*;

    #[test]
    fn test_numpages_assume_one_if_pages_not_available() {
        let response = HttpResponse::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        let num_pages = num_pages(&client, url, headers, operation).unwrap();
        assert_eq!(Some(1), num_pages);
    }

    #[test]
    fn test_numpages_error_on_404() {
        let response = HttpResponse::builder().status(404).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        assert!(num_pages(&client, url, headers, operation).is_err());
    }

    #[test]
    fn test_num_resources_assume_one_if_pages_not_available() {
        let headers = Headers::new();
        let response = HttpResponse::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines?page=1";
        let num_resources = num_resources(&client, url, headers, ApiOperation::Pipeline).unwrap();
        assert_eq!(30, num_resources.unwrap().delta);
    }

    #[test]
    fn test_num_resources_with_last_page_and_per_page_available() {
        let mut headers = Headers::new();
        // Value doesn't matter as we are injecting the header processor
        // enforcing the last page and per_page values.
        headers.set("link", "");
        let next_page = Page::new("https://gitlab.com/api/v4/projects/1/pipelines?page=2", 2);
        let last_page = Page::new("https://gitlab.com/api/v4/projects/1/pipelines?page=4", 4);
        let mut page_header = PageHeader::new();
        page_header.set_next_page(next_page);
        page_header.set_last_page(last_page);
        page_header.per_page = 20;
        let flow_control_header =
            FlowControlHeaders::new(Rc::new(Some(page_header)), Rc::new(None));
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_header)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://gitlab.com/api/v4/projects/1/pipelines?page=1";
        let num_resources = num_resources(&client, url, Headers::new(), ApiOperation::Pipeline)
            .unwrap()
            .unwrap();
        assert_eq!(80, num_resources.num);
        assert_eq!(20, num_resources.delta);
    }

    #[test]
    fn test_numresources_error_on_404() {
        let response = HttpResponse::builder().status(404).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        assert!(num_resources(&client, url, headers, operation).is_err());
    }
}

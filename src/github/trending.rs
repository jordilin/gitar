use regex::Regex;

use crate::{
    api_traits::{ApiOperation, TrendingProjectURL},
    cmds::trending::TrendingProject,
    http::Headers,
    io::{HttpRunner, Response},
    remote::query,
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> TrendingProjectURL for Github<R> {
    fn list(&self, language: String) -> Result<Vec<TrendingProject>> {
        let url = format!("https://{}/trending/{}", self.domain, language);
        let mut headers = Headers::new();
        headers.set("Accept".to_string(), "text/html".to_string());
        let response = query::get_raw::<_, String>(
            &self.runner,
            &url,
            None,
            headers,
            ApiOperation::SinglePage,
        )?;
        parse_response(response)
    }
}

fn parse_response(response: Response) -> Result<Vec<TrendingProject>> {
    let body = response.body;
    let proj_re = Regex::new(r#"href="/[a-zA-Z0-9_-]*/[a-zA-Z0-9_-]*/stargazers""#).unwrap();
    let description_re = Regex::new(r#"<p class="col-9 color-fg-muted my-1 pr-4">"#).unwrap();
    let mut descr_header_matched = false;
    let mut trending = Vec::new();
    let mut description = String::new();
    for line in body.lines() {
        if descr_header_matched {
            description = line.trim().to_string();
            descr_header_matched = false;
            continue;
        }
        if description_re.find(line).is_some() {
            descr_header_matched = true;
            continue;
        }
        if let Some(proj) = proj_re.find(line) {
            let proj = proj.as_str().split('"').collect::<Vec<&str>>();
            let proj_paths = proj[1].split('/').collect::<Vec<&str>>();
            if proj_paths[1] == "features" || proj_paths[1] == "about" || proj_paths[1] == "site" {
                continue;
            }
            let url = format!("https://github.com/{}/{}", proj_paths[1], proj_paths[2]);
            trending.push(TrendingProject::new(url, description.to_string()));
        }
    }
    Ok(trending)
}

#[cfg(test)]
mod test {

    use super::*;

    use crate::{
        setup_client,
        test::utils::{default_github, ContractType, ResponseContracts},
    };

    #[test]
    fn test_list_trending_projects() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_contract(200, "trending.html", None);
        let (client, github) = setup_client!(contracts, default_github(), dyn TrendingProjectURL);

        let trending = github.list("rust".to_string()).unwrap();
        assert_eq!(2, trending.len());
        assert_eq!("https://github.com/trending/rust", *client.url(),);
        assert_eq!(
            Some(ApiOperation::SinglePage),
            *client.api_operation.borrow()
        );
        let proj = &trending[0];
        assert_eq!("https://github.com/lencx/ChatGPT", proj.url);
        assert_eq!(
            "🔮 ChatGPT Desktop Application (Mac, Windows and Linux)",
            proj.description
        );
        let proj = &trending[1];
        assert_eq!("https://github.com/sxyazi/yazi", proj.url);
        assert_eq!(
            "💥 Blazing fast terminal file manager written in Rust, based on async I/O.",
            proj.description
        );
    }
}

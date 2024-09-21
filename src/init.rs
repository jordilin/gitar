use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::cli::init::InitCommandOptions;
use crate::error::{AddContext, GRError};
use crate::Result;

const CONFIG_TEMPLATE: &str = r#"
# Fill in the <VALUE> below with your own values
# and tweak accordingly.

# NOTE: Substitute domain '.' for '_' in the section name
# Ex. if DOMAIN is gitlab.com -> [gitlab_com]
# Ex. if DOMAIN is gitlab.example.com -> [gitlab_example_com]

[<DOMAIN>]

api_token="<VALUE>"
cache_location=".cache/gitar"


# Rate limit remaining threshold. Threshold by which the tool will stop
# processing requests. Defaults to 10 if not provided. The remote has a counter
# that decreases with each request. When we reach this threshold we stop for safety.
# When it reaches 0 the remote will throw errors.
rate_limit_remaining_threshold=10

[<DOMAIN>.merge_requests]

preferred_assignee_username="<VALUE>"
description_signature=""

# Array of usernames if the remote is Github
# Array of hashmaps username => user ID if the remote is Gitlab
# Ex:
# members = ["user1", "user2"]
# members = [{"username": "user1", "id": "1234"}, {"username": "user2", "id": "5678"}]

members = []

[<DOMAIN>.cache_expirations]

# Expire read merge requests in 5 minutes
merge_request="5m"
# Expire read project metadata, members of a project in 5 days
project="5d"
# Pipelines are read often, change often, so expire soon.
pipeline="30s"
# Container registry operations including listing image tags and repos
container_registry="1h"
# Expire read releases in 1 day
release="1d"
# Expire single page calls in 1 day. Ex. Trending repositories in github.com
single_page="1d"
# Expire your user gists in 1 day
gist="1d"
# Expire repository tags immediately
repository_tags="0s"

[<DOMAIN>.max_pages_api]

# Get up to 10 pages of merge requests when listing
merge_request=10
# Get up to 5 pages of project metadata, members of a project when listing
project=5
# Get up to 10 pages of pipelines when listing
pipeline=10
# Get up to 10 pages of container registry repositories when listing
container_registry=10
# Get up to 10 pages of releases when listing
release=10
# Get up to 5 pages of your gists
gist=5
# Get up to 10 pages of tags when listing
repository_tags=10

### Other domains - add more if needed
"#;

pub fn execute<P: AsRef<Path>>(options: InitCommandOptions, config_path: P) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(config_path.as_ref());

    let mut file = match file {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(GRError::PreconditionNotMet(format!(
                "Config file at {} already exists, move it aside before running `init` again",
                config_path.as_ref().display()
            ))
            .into())
        }
        Err(e) => {
            return Err(e).err_context(format!(
                "Unable to create config file at path {}",
                config_path.as_ref().display()
            ))
        }
    };
    generate_and_persist(options, &mut file).err_context(format!(
        "Failed to generate and persist config at path {}",
        config_path.as_ref().display()
    ))
}

fn generate_and_persist<W: Write>(options: InitCommandOptions, writer: &mut W) -> Result<()> {
    let data = change_placeholders(&options.domain);
    persist_config(data, writer)
}

fn persist_config<D: Into<String>, W: Write>(data: D, writer: &mut W) -> Result<()> {
    writer
        .write_all(data.into().as_bytes())
        .err_context("Writing the data to disk failed")?;
    Ok(())
}

fn change_placeholders(domain: &str) -> String {
    let domain = domain.replace(".", "_");
    CONFIG_TEMPLATE.replace("<DOMAIN>", &domain)
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_persist_config() {
        let options = InitCommandOptions {
            domain: "gitweb.com".to_string(),
        };
        let mut writer = Vec::new();
        let result = generate_and_persist(options, &mut writer);
        assert!(result.is_ok());
        assert!(writer.len() > 0);
        let content = String::from_utf8(writer).unwrap();
        assert!(content.contains("gitweb_com"));
    }
}

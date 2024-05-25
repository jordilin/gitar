# GitAR - Git All Remotes

[![Build status](https://github.com/jordilin/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/jordilin/gitar/actions)
[![codecov](https://codecov.io/gh/jordilin/gitar/graph/badge.svg)](https://codecov.io/gh/jordilin/gitar)

![GitAR](./logo.svg)

- [GitAR - Git All Remotes](#gitar---git-all-remotes)
  - [Installation](#installation)
  - [Usage](#usage)
    - [Configuration](#configuration)
    - [Example open a merge/pull request](#example-open-a-mergepull-request)
    - [Worth a thousand words](#worth-a-thousand-words)
  - [Remotes supported](#remotes-supported)
  - [Operations supported](#operations-supported)
    - [Merge requests](#merge-requests)
    - [Pipeline](#pipeline)
    - [Container registry](#container-registry)
    - [Project](#project)
    - [Browse remote using your browser](#browse-remote-using-your-browser)
    - [Releases](#releases)
    - [Auth User](#auth-user)
    - [Trending Repositories](#trending-repositories)
  - [Logging](#logging)
  - [Unit tests](#unit-tests)
  - [Gitar-Amps additional scripts and workflows](#gitar-amps-additional-scripts-and-workflows)
  - [License](#license)

Git multi-remote command line tool. Brings common development operations such as
opening a pull request down to the shell.

This is an alternative to both Github <https://github.com/cli/cli> and Gitlab
<https://gitlab.com/gitlab-org/cli> cli tools. The scope for now is
smaller. If you happen to use both Gitlab and Github and wanted to just have one
single tool, this can help.

Some benefits:

* It supports Gitlab and Github. One tool, to rul'em all.
* Written in Rust. Fast and Parallelizes operations to gather data locally and
  remotely.
* Common defaults. For example, the title of a pull requests is automatically
  set to the last commit. Defaults can be overriden when prompted.
* Caches API read calls. Common remote calls like gather project data that does
  not change often (project id, namespace, members), so subsequent calls are
  very fast.

I've only tested on MacOS and Linux.

## Installation

You can download the latest release from the releases page
<https://github.com/jordilin/gitar/releases> and place the binary anywhere in
your path.

Or you can build from source.

```bash
cargo build --release
./target/release/gr --help
```

## Usage

**WARNING**: Before using, I'd recommend to familiarize yourself in a test git
repository.

### Configuration

Place your configuration information in a file called `$HOME/.config/gitar/api`.
You'll need to gather a read/write API token from your Gitlab/Github account.

You can generate a new configuration file with the following command:

```bash
gr init --domain <domain>
```

Where `<domain>` is the domain of the remote. For example, `gitlab.com` or
`github.com`. This will create a new configuration file with some default
values.
Once created you can append new values for each domain you want.

Configuration follows a properties file format.

```
<domain>.property=value
```

Example configuration file:

```
# Gitlab.com
gitlab.com.api_token=<your api token>
gitlab.com.cache_location=/home/<youruser>/.cache/gr
gitlab.com.preferred_assignee_username=<your username>
gitlab.com.merge_request_description_signature=<your signature, @someone, etc...>

## Cache expiration configuration

# Expire read merge requests in 5 minutes
gitlab.com.cache_api_merge_request_expiration=5m
# Expire read project metadata, members of a project in 5 days
gitlab.com.cache_api_project_expiration=5d
# Pipelines are read often, change often, so expire immediately.
gitlab.com.cache_api_pipeline_expiration=0s
# Expire read container registry in 5 minutes
gitlab.com.cache_api_container_registry_expiration=5m
# Cache for reading releases
gitlab.com.cache_api_release_expiration=1d

## Max pages configuration

# Get up to 10 pages of merge requests when listing
gitlab.com.max_pages_api_merge_request=10
# Get up to 5 pages of project metadata, members of a project when listing
gitlab.com.max_pages_api_project=5
# Get up to 10 pages of pipelines when listing
gitlab.com.max_pages_api_pipeline=10
# Get up to 10 pages of container registry when listing
gitlab.com.max_pages_api_container_registry=10
# Get up to 10 pages of releases when listing
gitlab.com.max_pages_api_release=10

# Rate limit remaining threshold. Threshold by which the tool will stop
# processing requests. Defaults to 10 if not provided. The remote has a counter
# that decreases with each request. When we reach this threshold we stop for safety.
# When it reaches 0 the remote will throw errors.

gitlab.com.rate_limit_remaining_threshold=10

# Github
github.com.api_token=<your api token>
github.com.cache_location=/home/<youruser>/.cache/gr
github.com.preferred_assignee_username=<your username>
# github.com.merge_request_description_signature=@my-team

# Your company gitlab
gitlab.mycompany.com.api_token=<your api token>
...
```

Cache expiration configuration has three keys:

- `<domain>`.cache_api_merge_request_expiration: List merge_requests, get a
  merge request, etc... Any read operation involving merge/pull requests.
- `<domain>`.cache_api_project_expiration: Get project metadata, members of a
  project. This information does not change often, so a long expiration is fine.
- `<domain>`.cache_api_pipeline_expiration: List pipelines, get a pipeline, etc...

The values for these keys can accept any number followed by `s` for seconds, `m`
for minutes, `h` for hours, `d` for days. For example, `5m` means 5 minutes,
`5d` means 5 days, `0s` means immediate expiration.

If omitted, the default is immediate expiration, so read operations are always
pulled from the remote.

When listing merge requests, projects, pipelines, etc... the tool will fetch up
to max pages. We can control this per API as follows:

- `<domain>`.max_pages_api_merge_request: List merge_requests, get a
  merge request, etc... Any read operation involving merge/pull requests.
- `<domain>`.max_pages_api_project: Get project metadata, members of a project.
- `<domain>`.max_pages_api_pipeline: List pipelines, get a pipeline, etc...

If omitted, the default global number of pages for all APIs is 10. This is to
avoid fetching too much data when the amount of information is large.
The default number of results per page for Gitlab is 20 and for Github is 30.

### Example open a merge/pull request

Create a configuration file with an API read/write token as explained above.

```bash
gr mr create
```

* You are in a feature branch
* Prompt for assignee user
* Confirmation
* Open a merge request

### Worth a thousand words

[demo.webm](https://github.com/jordilin/gitar/assets/1031376/83a37d6e-e2eb-4b68-978e-816439b2c122)

## Remotes supported

Gitlab and Github.

## Operations supported

### Merge requests

In Gitlab they are known as merge requests and in Github as pull requests.

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| Open  | &#x2714; | &#x2714; |
| Approve | &#x2714; | &#x2716; |
| Merge | &#x2714; | &#x2714; |
| Get merge request details | &#x2714; | &#x2714; |
| List merge requests by their state | &#x2714;| &#x2714; |
| Close | &#x2714; | &#x2714; |
| Create comments on timeline | &#x2714; | &#x2714; |
| List comments on timeline | &#x2714; | &#x2714; |

### Pipeline

In Gitlab they are known as pipelines and in Github as actions.

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| List all pipelines | &#x2714; | &#x2714; |
| List pipeline runners | &#x2714; | &#x2716; |
| Get pipeline runner details | &#x2714; | &#x2716; |


### Container registry

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| List repositories | &#x2714; | &#x2716; |
| List tags | &#x2714; | &#x2716; |
| Get image metadata | &#x2714; | &#x2716; |

### Project

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| Get | &#x2714; | &#x2714; |

### Browse remote using your browser

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| Open git repo in browser | &#x2714; | &#x2714; |
| Open merge request in browser | &#x2714; | &#x2714; |
| Open pipeline in browser | &#x2714; | &#x2714; |

### Releases

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| List releases | &#x2714; | &#x2714; |
| List release assets | &#x2716; | &#x2714; |

### Auth User

Provided by the `gr my` command provides information about the user that holds
the auth token.

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| List assigned merge requests | &#x2714; | &#x2714; |
| List your projects | &#x2714; | &#x2714; |
| List your starred projects | &#x2714; | &#x2714; |


### Trending Repositories

| Operation | GitLab | GitHub |
| --------- | -------------- | -------------- |
| List by programming language | &#x2716; | &#x2714; |

All list operations support the following flags:

- `--page` to specify the page to fetch.
- `--from-page` and `--to-page` to specify a range of pages to fetch.
- `--num-pages` queries how many pages of data are available
- `--refresh` to force a refresh of the cache.
- `--sort` sorts data by date ascending or descending. Ascending is the default.
- `--created-after` and `--created-before` to filter by date if response
  payloads support `created_at` field.
- `--format` to specify the output format. Delimit fields by using a pipe, i.e. ` | ` is the default.

## Logging

Logging can be enabled by issuing the `--verbose` or `-v` global flag.

By default, INFO logs are enabled and will output to STDERR without interfering
with STDOUT in case you want to pipe the output to another command or file. INFO
will give just enough information to understand what is happening.

You can enable DEBUG logs by setting the `RUST_LOG` environment variable to
`debug`. DEBUG is way more verbose.

Ex: List all pipelines/actions with logging.

```bash
# INFO logs
gr --verbose pp list
# DEBUG logs
RUST_LOG=debug gr --verbose pp list
```

## Unit tests

JSON responses from Gitlab and Github are verified in the contracts folder.
Those are used to generate mock responses for unit tests.

```bash
cargo test
```

## Gitar-Amps additional scripts and workflows

Gitar-Amps are wrapper scripts that make use of gitar in order to provide
additional workflows and use cases.
It is a companion project that can be found at
<https://github.com/jordilin/gitar-amps>

## License

This project is licensed under

- Source code: MIT license ([LICENSE](LICENSE) or
  [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

- GitAR logo: [Creative Commons
Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)](https://creativecommons.org/licenses/by-nc-sa/4.0/)

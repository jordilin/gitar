# GitAR - Git All Remotes

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

```bash
cargo build --release
./target/release/gr --help
```

## Remotes

Gitlab and Github.

## Operations supported in all remotes

* Open/Merge/List/Close a pull request
* Browse repository, merge request
* Clone remote feature branch locally

## Configuration

Configuration follows a properties file format.

```
<domain>.property=value
```

Example configuration file:

```
# Gitlab.com
gitlab.com.api_token=<your api token>
gitlab.com.cache_location=/home/<youruser>/.cache/gg
gitlab.com.preferred_assignee_username=<your username>
gitlab.com.merge_request_description_signature=<your signature, @someone, etc...>

# Github
github.com.api_token=<your api token>
github.com.cache_location=/home/<youruser>/.cache/gg
github.com.preferred_assignee_username=<your username>
# github.com.merge_request_description_signature=@my-team

# Your company gitlab
gitlab.mycompany.com.api_token=<your api token>
...
```

## Opening a merge request with gr

Create a configuration file with an API read/write token as explained above.

```bash
gr mr create
```

* You are in a feature branch
* It will fetch latest upstream origin/default-target-branch
* It will rebase to your feature branch
* Prompt for assignee user
* Confirmation
* Open a merge request

## Unit tests

JSON responses from Gitlab and Github are verified in the contracts folder.
Those are used to generate mock responses for unit tests.

```bash
cargo test
```

## License

This project is licensed under

* MIT license ([LICENSE](LICENSE) or
  [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

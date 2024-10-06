# Configuration

<!-- toc -->

## A quick note about API calls and pages

When I talk about an API call or number of pages, I'm referring to actual
HTTP requests to the Github or Gitlab API. Hence, when I say an API call or one
page of information, I'm referring to a single HTTP request.

## Create a configuration file (Optional)

In order to create a configuration file, you can run the following command. In
this example we are setting the domain to `github.com`. It can be `gitlab.com`
or any other domain that you want to use, for example your own company's domain.

```bash
gr init --domain github.com
```

This will create a configuration file in your `$HOME/.config/gitar/gitar.toml`
directory with some defaults. The configuration follows a TOML file format.
Once the file is created, open the configuration file to add your
API token and the optional sections.

## TOML domain sections

The configuration file is divided into sections. Each section is named after the
domain you are targetting. The formatting of the sections is as follows. Dots in
the domain name are replaced by underscores.

```toml
[ github_com ]
api_token="<your token>"

[ gitlab_com ]
api_token="<your token>"

[ gitlab_yourcompany_com ]
api_token="<your token>"
```

### No configuration file

Potential use cases: CI/CD pipelines, automation scripts, one-off runs.

If no configuration file is provided, then gitar expects an authentication token
environment variable to be set. Check the [API token](#api-token) section for
more information.

No configuration means that there won't be any caching for read operations either.

## API token

Gitar needs an API token to access the Github or Gitlab API. The token can be
set by using an environment variable or by placing it in the configuration file.

### Environment variable

The environment variable needs to be named after the domain you are targeting.
For example, if the remote is `gitlab.com` the environment variable should be
named `GITLAB_API_TOKEN` and if the remote is `github.com` the environment
variable should be named `GITHUB_API_TOKEN`. If you have a subdomain such as
`gitlab.yourcompany.com`, the environment variable should be named
`GITLAB_YOURCOMPANY_API_TOKEN`. Finally, if you are using a non FQDN domain such as
`mygitlab`, then the environment variable should be `MYGITLAB_API_TOKEN`.

This can be summarized in the following table:

| Domain | Environment variable |
|--------|----------------------|
| github.com | GITHUB_API_TOKEN |
| gitlab.com | GITLAB_API_TOKEN |
| gitlab.yourcompany.com | GITLAB_YOURCOMPANY_API_TOKEN |
| mygitlab | MYGITLAB_API_TOKEN |

If the environment variable is not set, then Gitar will look for the token in
the configuration file as explained in the following sections.

### Github.com

To get an API token for Github, go to your Github account settings -> Developer
settings -> Personal access tokens -> Tokens (classic)
At the time of writing, the URL is <https://github.com/settings/tokens>

Create a new token with the scopes: `repo`, `user`, `project`, `gist`. By
clicking on each scope check box it will automatically select all the sub-scopes
under it. Then copy the token and place it in the configuration file. You'll see
a line like:

```verbatim
[ github_com ]
api_token="<your-token>"
```

### Gitlab.com

To get an API token for Gitlab, go to your Gitlab account settings -> Access
tokens and create an api token. Current URl at the time of writing is
<https://gitlab.com/-/user_settings/personal_access_tokens> Select the `api`
scope, give it a name and an expiration date. Click on `Create personal access
token` and copy the token over to the configuration file.

```verbatim
[ gitlab_com ]
api_token="<your-token>"
```

## Merge requests configuration section

The assignee username is the username that will be used to automatically assign
a pull request to. Normally, that would be your username. Example, whenever I
create a pull request to my own repository, I automatically assign it to myself.
Members are the members you want to potentially assign a merge request to.
For Gitlab, both members and assignee need to be formatted with a map of
username and user ID. For Github, only the username is needed but the user ID
can also be provided. Gitar can pull this information directly from the API:

```bash
# Retrieve number of pages required to retrieve candidates for assignee assignment
gr pj members --num-pages
# You might want to bypass throttling if num pages is just a few of them (<10)
gr pj members --from-page 1 --to-page <total-pages> --throttle 2000 --format toml | tee members.toml
# Retrieve my username metadata.
gr us get <my-username> --format toml | tee my-username.toml
```

The output can be pasted to the configuration file.
NOTE: The user ID number can be placed in between quotes or without them.

```toml
[ github_com.merge_requests ]
preferred_assignee_username={ "username" = "<your-github-username>", "id" = <your-github-user-id> }
members = [
  { username = "user1", id = "1234" },
  { username = "user2", id = "5678" },
  { username = "user3", id = "9012" }
]

[ gitlab_com.merge_requests ]
preferred_assignee_username={ "username" = "<your-gitlab-username>", "id" = <your-gitlab-user-id> }
members = [
  { username = "user1", id = "1234" },
  { username = "user2", id = "5678" },
  { username = "user3", id = "9012" },
]
```

### Per project merge request configurations

If you want to have different members in different projects, you can do so by
adding the following section `[<domain>.merge_requests.<group>_<project_name>]`.
Basically the path `/` is replaced by `_`. Ex. `jordilin/gitar` becomes
`jordilin_gitar`. Same for subgroups: `group_subgroup_projectname`.

Example:

```toml
[ github_com.merge_requests.jordilin_gitar ]
members = []
```

This will effectively override the global configuration for the domain.

## API types and their configurations

Gitar groups API calls into different types taking full control on how we want
to retrieve information and how long it is going to be cached. Why is that? The
reason is that as project owners or collaborators of the projects we work on, we
know in advance the rate of change. Project information such as its members,
don't get added or removed on a daily basis, so we can cache that information
for a long time. On the other hand, the status of a pipeline, releases, merge
requests change more often. The number of pages to retrieve per API can also be
adjusted. Please see section [caching](./caching.md) for more information.

API types:

- Project
- Merge request
- Pipeline
- Release
- Container registry
- Repository tags

### Maximum pages to retrieve per API type

One page equals to one HTTP request. Gitar has an internal default of 10 maximum
pages that can be retrieved per API call. This takes effect on list operations
in every subcommand that has listing support. This can be increased/decreased on
a per API basis. This information needs to be set in the TOML section
`[domain.max_pages_api]`. Ex. `[github_com.max_pages_api]`.

- `project=<number>` This API type is used to retrieve information
  about a project/repository such as its members. When opening a merge request
  gitar will pull up to `project` pages of members to find the
  your username to assign the pull request to. If you get an error that your
  username cannot be found, increase this number. Once the members have been
  retrieved, the list is permanently cached for next calls, so it will be fast.

- `merge_request=<number>` This API type is used to retrieve
  information about pull/merge requests. For example, listing opened, merged,
  closed pull requests, etc...

- `pipeline=<number>` This API type is used to retrieve information
  about CI/CD pipelines/actions that run in the given project. This takes place
  in list operations in the `pp` subcommand.

- `release=<number>` This API type is used to retrieve information
  about releases in the current project, such as listing releases and its
  assets.

- `container_registry=<number>` This API type is used to retrieve
  information about container registry images in the current project. This is
  supported in Gitlab only. This takes place in list operations in the `dk`
  subcommand.

- `repository_tags=<number>` This API type is used to
  retrieve information about tags in a repository. This takes place when listing
  repository tags using the `gr pj tags` subcommand.

### Local cache duration for each API type

Gitar has local caching support for each API type. Every HTTP response
is cached and the next time the same request is made, the same response is
returned until expired. The responses are stored in a local cache directory
which can be configured by setting the cache location in the `[<domain>]` section

- `cache_location="<full-path-to-cache-directory>"` The path needs to exist
  and be writable by the user running the gitar command.

Cache values are a number followed by a letter representing the time unit. For
example `5m` means 5 minutes, `5d` means 5 days, `30s` means 30 seconds. The
units supported are `s` for seconds, `m` for minutes, `h` for hours, `d` for
days. A cache value of `0` followed by a time unit means automatic expiration of
the cache. In that case, gitar will contact the remote API doing a conditional
HTTP request to check if the cache is still valid and return the cached response
if it is. Otherwise, it will automatically update the cache with the new
response.

Cache duration for each API type can be set in the TOML section
`[<domain>.cache_expirations]`. Ex. `[github_com.cache_expirations]`.

- `merge_request="<number><time-unit>"` This API type is
  used to retrieve information about pull/merge requests. For example, listing
  opened, merged, closed pull requests.

- `project="<number><time-unit>"` This API type is
  used to retrieve information about a project/repository such as its members.
  When opening a merge request gitar will pull up to `max_pages_api_project`
  pages information to retrieve project information and its members. Project
  information does not change often, so a higher cache value of a few days can
  be ok. Members of a project, project ID, etc... can be cached for longer time
  depending on the projects you work on.

- `pipeline="<number><time-unit>"` This API type is used
  to retrieve information about CI/CD pipelines/actions that run in the given
  project. A low cache value is recommended for this API type as the status of
  pipelines change often in projects.

- `"container_registry="<number><time-unit>"` This
  API type is used to retrieve information about container registry images in
  the current project. This is supported in Gitlab only. This takes place in
  list operations in the `dk` subcommand.

- `release="<number><time-unit>"` This API type is
  used to retrieve information about releases in the current project, such as
  listing releases and its assets.

- `single_page="<number><time-unit>"` This API type
  is used to retrieve information about single page calls. For example, trending
  repositories in github.com. A value of `1d` is recommended for this API type.

- `repository_tags="<number><time-unit>"` This API
  type is used to retrieve information about tags in a repository.

>**Note**: Local cache can be automatically expired and refreshed by issuing the
`-r` flag when running the `gr` command.

## Split configuration files

If you have merge request configuration for multiple projects, multiple
domains, the main configuration file `gitar.toml` can quickly grow in size.
To avoid this, you can split the configuration file into multiple files as
follows. Gitar reads the main configuration file `gitar.toml` and then attempts
to read the following file name patterns in the same directory:

- `<domain>.toml` Ex: `github_com.toml`, `gitlab_com.toml`, `gitlab_yourcompany_com.toml`
- `<domain>_<group>_<project>.toml` Ex: `github_com_jordilin_gitar.toml`, `gitlab_com_group_subgroup_projectname.toml`

As we can observe in the examples above, the following conventions are used:

1. Substitute `.` with `_`.
2. Substitute `/` with `_` for domain, group, and project names.

The total configuration is the concatenation of all the files. For example, if
we have `gitar.toml` and `github_com.toml` in the same directory, then gitar
will read both files and concatenate the configuration. If there are duplicate
sections it will throw a TOML configuration error. Sections can be added in any
of the files. For example, if you were to specify the `api_token` for Github in
`github_com.toml` adding it to the `gitar.toml` file would be an error.
If you prefer, you can also keep one configuration for each domain and remove
the main `gitar.toml` file.

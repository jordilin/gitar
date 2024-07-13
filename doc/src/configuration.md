# Configuration

<!-- toc -->

## A quick note about API calls and pages

When I talk about an API call or number of pages, I'm referring to actual
HTTP requests to the Github or Gitlab API. Hence, when I say an API call or one
page of information, I'm referring to a single HTTP request.

## Create a configuration file

In order to create a configuration file, you can run the following command. In
this example we are setting the domain to `github.com`. It can be `gitlab.com`
or any other domain that you want to use, for example your own company's domain.

```bash
gr init --domain github.com
```

This will create a configuration file in your `$HOME/.config/gitar/api`
directory with some defaults. The configuration follows a properties file format
with key, value pairs. Once the file is created, open the api file to add your
API token and the preferred assignee username you want to use in your pull
requests (traditionally your own username).

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
github.com.api_token=<your-token>
```

### Gitlab.com

To get an API token for Gitlab, go to your Gitlab account settings -> Access
tokens and create an api token. Current URl at the time of writing is
<https://gitlab.com/-/user_settings/personal_access_tokens> Select the `api`
scope, give it a name and an expiration date. Click on `Create personal access
token` and copy the token over to the configuration file.

```verbatim
gitlab.com.api_token=<your-token>
```

## Assignee username

The assignee username is the username that will be used to automatically assign
a pull request to. Normally, that would be your username. Example, whenever I
create a pull request to my own repository, I automatically assign it to myself.

```verbatim
github.com.preferred_assignee_username=<your-github-username>
```

When targetting other repositories outside of your namespace, .i.e creating a
pull request from your fork to the original repository, the assignee is left
blank.

## API types and their configurations

Gitar groups API calls into different types taking full control on how we want
to retrieve information and how long it is going to be cached. Why is that? The
reason is that as project owners or collaborators of the projects we work on, we
know in advance the rate of change. Project information such as its members,
don't get added or removed on a daily basis, so we can cache that information
for a long time. On the other hand, the status of a pipeline, releases, merge
requests change more often. The number of pages to retrieve per API can also be
adjusted.

API types:

- Project
- Merge request
- Pipeline
- Release
- Container registry

### Maximum pages to retrieve per API type

One page equals to one HTTP request. Gitar has an internal default of 10 maximum
pages that can be retrieved per API call. This takes effect on list operations
in every subcommand that has listing support. This can be increased/decreased on
a per API basis.

- `<domain>.max_pages_api_project=<number>` This API type is used to retrieve information
  about a project/repository such as its members. When opening a merge request
  gitar will pull up to `max_pages_api_project` pages of members to find the
  your username to assign the pull request to. If you get an error that your
  username cannot be found, increase this number. Once the members have been
  retrieved, the list is permanently cached for next calls, so it will be fast.

- `<domain>.max_pages_api_merge_request=<number>` This API type is used to retrieve
  information about pull/merge requests. For example, listing opened, merged,
  closed pull requests, etc...

- `<domain>.max_pages_api_pipeline=<number>` This API type is used to retrieve information
  about CI/CD pipelines/actions that run in the given project. This takes place
  in list operations in the `pp` subcommand.

- `<domain>.max_pages_api_release=<number>` This API type is used to retrieve information
  about releases in the current project, such as listing releases and its
  assets.

- `<domain>.max_pages_api_container_registry=<number>` This API type is used to retrieve
  information about container registry images in the current project. This is
  supported in Gitlab only. This takes place in list operations in the `dk`
  subcommand.

### Local cache duration for each API type

Gitar has local caching support for each API type. Every HTTP response
is cached and the next time the same request is made, the same response is
returned until expired. The responses are stored in a local cache directory
which can be configured by setting:

- `<domain>.cache_location=<full-path-to-cache-directory>` The path needs to exist
  and be writable by the user running the gitar command.

Cache values are a number followed by a letter representing the time unit. For
example `5m` means 5 minutes, `5d` means 5 days, `30s` means 30 seconds. The
units supported are `s` for seconds, `m` for minutes, `h` for hours, `d` for
days. A cache value of `0` followed by a time unit means automatic expiration of
the cache. In that case, gitar will contact the remote API doing a conditional
HTTP request to check if the cache is still valid and return the cached response
if it is. Otherwise, it will automatically update the cache with the new
response.

Cache duration for each API type is as follows:

- `<domain>.cache_api_merge_request_expiration=<number><time-unit>` This API type is
  used to retrieve information about pull/merge requests. For example, listing
  opened, merged, closed pull requests.

- `<domain>.cache_api_project_expiration=<number><time-unit>` This API type is
  used to retrieve information about a project/repository such as its members.
  When opening a merge request gitar will pull up to `max_pages_api_project`
  pages information to retrieve project information and its members. Project
  information does not change often, so a higher cache value of a few days can
  be ok. Members of a project, project ID, etc... can be cached for longer time
  depending on the projects you work on.

- `<domain>.cache_api_pipeline_expiration=<number><time-unit>` This API type is used
  to retrieve information about CI/CD pipelines/actions that run in the given
  project. A low cache value is recommended for this API type as the status of
  pipelines change often in projects.

- `<domain>.cache_api_container_registry_expiration=<number><time-unit>` This
  API type is used to retrieve information about container registry images in
  the current project. This is supported in Gitlab only. This takes place in
  list operations in the `dk` subcommand.

- `<domain>.cache_api_release_expiration=<number><time-unit>` This API type is
  used to retrieve information about releases in the current project, such as
  listing releases and its assets.

- `<domain>.cache_api_single_page_expiration=<number><time-unit>` This API type
  is used to retrieve information about single page calls. For example, trending
  repositories in github.com. A value of `1d` is recommended for this API type.

>**Note**: Local cache can be automatically expired and refreshed by issuing the
`-r` flag when running the `gr` command.

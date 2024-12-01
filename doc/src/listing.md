# The list subcommand

<!-- toc -->

The list subcommand is used to pull data from specific resources such as
pipelines and merge requests. Gitar implements best practices to avoid being
rate limited, caches responses and uses pagination to pull the required data
using the `--from-page` and `--to-page` flags.

## Auto throttling

Gitar will automatically throttle the requests after three consecutive HTTP
calls have been made. The throttling is based on the rate limit headers plus a
jitter interval between 1 and 5 seconds. The user can also specify a fixed
throttle interval with `--throttle` or a random one with `--throttle-range`.

## Max pages to fetch

If no configuration is provided, the default is a max of 10 pages. This can be
overridden with `--to-page` where it will fetch up to the specified page or a
range of pages with `--from-page` and `--to-page`.

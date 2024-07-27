# Gitar's caching configuration approach

Every HTTP API call to Gitlab or Github is categorized into different API
operation types. Each operation type has its own cache duration and that is
defined by the user in the configuration file. The reason for this is that some
resources like the project members in the repositories that you collaborate on
might not change often, while pipelines and merge requests change way more
often. If you setup the cache to be "0<time_unit>" whichever time unit you want
(e.g 0s, 0m, 0h, 0d), then regular HTTP caching mechanisms will take place. In
this case, Gitar will cache and then inspect the cache-control header and its
directives to determine the cache state and if it should be invalidated or not.
While it will perform better than no cache, it won't perform as fast as just
immediately returning the cached response as mandated by the user. If you know
up front that some resources don't change often, you can set the cache duration
to a higher value and then Gitar will return the cached response immediately
without making additional HTTP calls.

Use cases:

- Opening merge requests, project information can be cached for a long time
  making assignee lookups nearly immediate
- Data extraction/experimentation. If you are going to gather release data,
  merge requests, etc... you can cache the responses for a long time for faster
  experimentation.

## Evaluation order of cache duration

1. Look for the API type specific cache duration (determined by the user)
2. If not found or configured to be "0<time_unit>", then inspect the
cache-control header and its directives to determine the cache state.

All in all, the user is in full control for how long the cache should be kept for
while still respecting HTTP cache control mechanisms.

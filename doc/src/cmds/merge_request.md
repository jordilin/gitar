# gr mr

`gr mr` is probably one of the most used commands in `gitar` as it allows you to
open and handle merge requests from the command line. It supports several
subcommands that allow you to list, create, update, and merge merge requests.

## Open a merge request

In its most basic form, you just create a new merge request with the following command:

```bash
gr mr create
```

This assumes you are in a feature branch and you want to merge it into the
default branch in origin. The command will prompt you for the title,
description, assignee and finally confirm if you want to create a merge request.

# Gitar commands

<!-- toc -->

## Commands available

- [Merge requests](./merge_request.md)
- [Pipelines](./pipeline.md)
- [Amps](./amps.md)

All gitar commands have a set of common options that can be used to control
their behavior.

## Global options

- `--help` - Show help message and exit.
- `--version` - Show version information and exit.
- `--verbose` - Enable logging of debug messages. This is useful for debugging
  issues with the tool. Log traces are written to the standard error output.

## List options

List options control the behavior of `gitar` commands that list resources. They
enable throttling, pagination and sorting of the output.

They can be found under the `List options` section when issuing `--help`.

A resource such as a pipeline in Gitlab or action in Github can have a large
number of items. The list options allows us to retrieve just one page, or a
subset of the items by controlling the `--from-page` and `--to-page` options.

Useful options when listing resources are:

- `--num-pages` The total number of pages available to retrieve. If the resource
  contains lots of items, we can issue gitar with throttling enabled in order to
  avoid hitting the API rate limit.

# gr pp

`gr pp` is a command that allows you to handle pipelines from the command line.

## List pipelines

To list pipelines for the current project, you can use the following command:

```bash
gr pp list
```

## Lint pipeline configuration (`.gitlab-ci.yml`)

To lint the pipeline configuration file (`.gitlab-ci.yml`), you can use the following command:

```bash
gr pp lint
```

## Get runners available for the project (Gitlab)

To get the runners available for the project, you can use the following command:

```bash
gr pp rn list <status>
```

Where `<status>` can be one of the following values:

- `online`
- `offline`
- `stale`
- `never-contacted`
- `all`

## Get the merged .gitlab-ci.yml

In the scenario where you use a Gitlab pipeline declared in `.gitlab-ci.yml` and
the pipeline contains `include` statements, you can use the following command to
obtain the total configuration of the pipeline:

```bash
gr pp merged-ci
```

This will print out to the console the total merged `.gitlab-ci.yml` file that
includes all the contents from the included yaml files. If the pipeline has
errors it will print out the errors if any.

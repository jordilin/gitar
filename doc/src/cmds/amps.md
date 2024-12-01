# gr amps

`gr amps` lists and execute amps. Amps are wrappers around the `gr` command
itself. Amps normally execute gr subcommands and perform additional logic to get
to the desired result. They can also be seen as just `gr` scripts that can be
executed from `gr` itself. A curated list of amps is provided at
<https://github.com/jordilin/gitar-amps>.

## List available amps

```bash
gr amps
```

or

```bash
gr amps list
```

## Execute an amp in-line

To execute an amp in-line, you can use the following command:

```bash
gr amps exec "<amp-name> <arg_0> <arg_1> ... <arg_n>"
```

For example:

```bash
gr amps exec "list-last-assets github.com/jordilin/gitar"
```

will print out the URLs of the last stable release assets for the
`github.com/jordilin/gitar` repository.

**> Note:** Arguments for the amps are optional and the amp name and its
arguments should be enclosed in double quotes.

## Execute an amp by prompt

```bash
gr amps exec
```

This command will prompt you to select an amp from the list of available amps.
After selecting the amp name, it will prompt you to enter the arguments for the
amp. Upon pressing enter, the amp will be executed.

The prompt understands the following prompt queries once an amp has been
selected:

- `h` or `help` - Show help message for the selected amp.
- `q` or `quit` - Quit the prompt and return back to the CLI.

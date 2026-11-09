# Report files

Besides what it prints, a run writes its results as files, for tools and for reading later, as the
Java edition does. Every run writes them except a run of some mutants (`--only`, and so `verify`
and `explain`), which says nothing about the rest. The summary's `Reports` line lists the files it
wrote, as `run-finished`'s `reports` does.

## Formats

| Kind | File | What it is |
|---|---|---|
| `json` | `mutation.json` | The [mutation-testing report schema](https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema), version 2: each source file with its text and its mutants, their status, the tests that cover them, and why each has its status. Other tools that read that schema read it. |
| `html` | `mutation.html` | The same report in the schema's own viewer. Everything is in the one file, so it opens from disk with no network. |

`json` and `html` are written unless you choose otherwise.

`language` is `rust` in each file's entry, and the tests are grouped by the module that holds them
(`tests`, `pricing`), where the Java edition groups them by class.

## Where they go

In `target/mutastic` beside the build, wherever your target directory is. A file given a path of
its own goes there instead, relative to the package's directory.

## Choosing which

`--report=KIND[:PATH]` names one report. Given at all, it replaces the defaults, so name each one
you want; it can be given more than once.

- `--report=json` writes only `mutation.json`.
- `--report=json:reports/mutation.json` writes the JSON report to `reports/mutation.json`.
- `--report=none` writes nothing.

```sh
mutastic run fixtures/pricing --report=json
```

See [`--report`](commands.md#--report) for the option itself.

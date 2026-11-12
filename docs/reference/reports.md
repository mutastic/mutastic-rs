# Report files

Besides what it prints, a run writes its results as files, for tools and for reading later, as the
Java edition does: a report viewer to browse every mutant beside its code, a dashboard or a badge
that tracks the score from run to run, a CI step that keeps the report with the build. Every run writes them except a run of some mutants (`--only`, and so `verify`
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

### What `mutation.json` holds

The pricing fixture's, trimmed to its one missed mutant, without the file's text (`source`), the
other mutants, the tests, its absolute `projectRoot` or its timings:

<!-- output: reference/mutation-json -->
```text
{
  "$schema": "https://raw.githubusercontent.com/stryker-mutator/mutation-testing-elements/master/packages/report-schema/src/mutation-testing-report-schema.json",
  "schemaVersion": "2",
  "thresholds": {
    "high": 80,
    "low": 60
  },
  "files": {
    "src/lib.rs": {
      "language": "rust",
      "source": "…",
      "mutants": [
        {
          "id": "mb2f8ae57",
          "mutatorName": "boundary",
          "replacement": ">",
          "description": "`>=` becomes `>`",
          "location": {
            "start": {
              "line": 26,
              "column": 18
            },
            "end": {
              "line": 26,
              "column": 20
            }
          },
          "status": "Survived",
          "statusReason": "no test checks discount() at total == 100",
          "coveredBy": [
            "tests::small_orders_pay_full_price",
            "pricing::big_orders_get_ten_percent_off"
          ],
          "testsCompleted": 2
        }
      ]
    }
  },
  "framework": {
    "name": "mutastic",
    "version": "1.0.0"
  }
}
```

`location` is 1-based, its end one past the change. `statusReason` is the card's headline, and
`coveredBy` the tests that ran it and passed.

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

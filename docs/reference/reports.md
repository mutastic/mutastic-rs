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
| `sarif` | `mutation.sarif` | SARIF 2.1.0, which code scanning reads: GitHub's alerts on a pull request's lines, Azure DevOps, and SARIF viewers in an editor. |

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

## Code scanning

`--report=sarif` writes a result for each missed mutant, as a warning, and each one no test
reaches, as a note, on the line it's on, with the card's headline as its message: `no test checks
discount() at total == 100`. Caught and skipped mutants aren't results. Each operator that made a
result is a rule, which links to the operator's row in [Operators](../operators.md#every-operator),
or says what a rules file's operator does in the file's own words.

A result carries the mutant's id in `partialFingerprints` (`mutasticId/v1`), and the id stays the
same when code above it moves, so an alert follows its mutant from one run to the next instead of
closing and opening again. A result never suggests a fix: the fix is a test, never a change to the
code. Paths are relative to the git repository the workspace is in, which is what code scanning
resolves them against, so a package in a folder of the repository points at the right files.

On GitHub, upload it after the run. With `--since`, only the lines a pull request changed are
mutated, so its alerts are its own; `--threshold=0` keeps the job green, leaving the alerts to say
what's missed:

```yaml
    permissions:
      security-events: write
    steps:
      # … check out with fetch-depth: 0 and install mutastic, as in Running in CI
      - run: mutastic run --since=origin/${{ github.base_ref }} --report=sarif --threshold=0
      - uses: github/codeql-action/upload-sarif@9b3254085494b55a0f7c4c2b3d2d52cc712ce370 # v4.39.1
        with:
          sarif_file: target/mutastic/mutation.sarif
          category: mutastic
```

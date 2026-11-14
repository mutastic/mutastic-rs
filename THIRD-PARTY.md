# Third-party code

mutastic includes the following, unchanged, under its own licence, as the Java edition does.

| What | Version | From | Licence | Where in mutastic |
|---|---|---|---|---|
| Bricolage Grotesque (variable, Latin) and JetBrains Mono (400 and 700, Latin), the HTML report's fonts, embedded in its page (`mutation.html`), as the Java edition's | 5.3.0 of the Fontsource packages `@fontsource-variable/bricolage-grotesque` and `@fontsource/jetbrains-mono` | https://fontsource.org/fonts/bricolage-grotesque, https://fontsource.org/fonts/jetbrains-mono, by way of mutastic's `report-ui/fonts/`, with their licences | SIL Open Font License 1.1 | `crates/mutastic/src/report/page/report.html`, the page vendored from mutastic |
| mutation-testing-report-schema, the schema `mutation.json` follows: in tests only, to check it, and not shipped | 3.9.0 | https://unpkg.com/mutation-testing-report-schema@3.9.0/src/mutation-testing-report-schema.json (SHA-256 `404575e9264686dbf85c399f6531fdb489bebc3146e7e1d793b8083bc6a041ae`) | Apache License 2.0 | `crates/mutastic/tests/schema/report/`, with its `LICENSE` |
| SARIF 2.1.0 JSON Schema, the format `mutation.sarif` follows: in tests only, to check it, and not shipped | 2.1.0, OASIS Standard with Errata 01 | https://json.schemastore.org/sarif-2.1.0.json (SHA-256 `c96eb2d311c37b0a38cbd18c52d79a68f778bbd2d831abed7412d9850740f785`), the schema published by the OASIS SARIF TC (oasis-tcs/sarif-spec) | © OASIS Open, under the OASIS Intellectual Property Rights Policy | `crates/mutastic/tests/schema/sarif-2.1.0.json` |

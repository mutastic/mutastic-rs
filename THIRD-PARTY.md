# Third-party code

mutastic includes the following, unchanged, under its own licence, as the Java edition does.

| What | Version | From | Licence | Where in mutastic |
|---|---|---|---|---|
| mutation-testing-elements, the report viewer the HTML report embeds | 3.9.0 | https://unpkg.com/mutation-testing-elements@3.9.0/dist/mutation-test-elements.js (SHA-256 `751fb010242b0b44e32d84fe7fe0b9ff1da182823b94f59f5c52b001fcfc163b`) | Apache License 2.0 | `crates/mutastic/src/report/viewer/`, with its `LICENSE` |
| mutation-testing-report-schema, the schema `mutation.json` follows: in tests only, to check it, and not shipped | 3.9.0, the viewer's | https://unpkg.com/mutation-testing-report-schema@3.9.0/src/mutation-testing-report-schema.json (SHA-256 `404575e9264686dbf85c399f6531fdb489bebc3146e7e1d793b8083bc6a041ae`) | Apache License 2.0 | `crates/mutastic/tests/schema/report/`, with its `LICENSE` |
| SARIF 2.1.0 JSON Schema, the format `mutation.sarif` follows: in tests only, to check it, and not shipped | 2.1.0, OASIS Standard with Errata 01 | https://json.schemastore.org/sarif-2.1.0.json (SHA-256 `c96eb2d311c37b0a38cbd18c52d79a68f778bbd2d831abed7412d9850740f785`), the schema published by the OASIS SARIF TC (oasis-tcs/sarif-spec) | © OASIS Open, under the OASIS Intellectual Property Rights Policy | `crates/mutastic/tests/schema/sarif-2.1.0.json` |

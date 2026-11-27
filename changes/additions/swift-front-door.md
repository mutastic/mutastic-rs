- **A Swift project is told the Swift edition isn't released yet.** `mutastic` in a directory with
  `Package.swift`, an `.xcodeproj` or an `.xcworkspace`, or below one, stopped looking for a Cargo
  project and said there wasn't one. It now stops with `swift-edition`: what the project is, that
  the Swift edition is planned for 1.5, and how to run one built from its source,
  `MUTASTIC_SWIFT_ENGINE` naming its `mutastic`, which gets the command as the Java edition does.
  `--edition=swift` chooses it, and `Cargo.toml` beside `Package.swift` is `both-editions`, as
  beside `pom.xml`. A coding agent's `mutastic gate` there still lets the agent go
  (`docs/editions.md`).

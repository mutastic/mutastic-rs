- **The agent format quotes a cfg the build doesn't have inside the fence.** A cfg's value is free
  text, so `#[cfg(target_os = "…")]` could put a sentence of the project's on the `not built here:`
  line, which came before `<<mutastic-data …>>`, where an agent reads mutastic's own words. The
  line is now inside the fence, which opens for it alone when nothing was missed
  ([docs/concepts/output.md](docs/concepts/output.md#agent-output-and-untrusted-text)). The person's
  format and `mutastic mutants` show it as before.

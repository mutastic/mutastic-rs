- **The output kit is a crate of its own.** Colour roles, glyphs and labels, escaping and the
  agent's fence, close names, help's layout, progress's bar and heartbeat, and the events' envelope
  moved to `crates/mutastic-output`, which depends on none of the workspace's crates, as
  `tests/layers.rs` checks, so proptastic can share them without mutastic. What mutastic prints is
  unchanged.

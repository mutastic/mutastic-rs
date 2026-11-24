- **The build's everyday commands are one word each, with [just](https://just.systems).** `just`
  lists them; `just check` is everything a pull request needs to pass (the format, clippy and the
  tests), and `just ci` is what CI runs. The words mean the same in the Java edition, and a move off
  cargo changes the recipes beneath them, not the words people and agents type.

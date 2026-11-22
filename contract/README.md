# The contract's pins

`tests/contract.rs` holds this edition to the editions' contract, which lives in the Java
repository. [`source.json`](source.json) says where: the repository and the file in it. The commit
the check reads it at is the newest pin in [`pins/`](pins).

When the commit was one line of one file, every pull request that moved it changed that line, so
any two open together conflicted, and each merge sent the rest back to be pinned again. So each
pin is a file of its own, and the check reads the newest.

## Adding a pin

A pull request that needs a later contract (an option added, a difference recorded or closed)
adds a file `contract/pins/<slug>.json`, the slug a few words of your own in lower case with
hyphens:

```sh
scripts/pin-contract.sh take-operators-as-java            # mutastic main, as it is now
scripts/pin-contract.sh take-operators-as-java 1f83d4d    # a commit, or a branch
```

```json
{
  "commit": "86480200c9c19ef7c49905e82255673def377770",
  "date": "2026-11-20T10:30:52Z"
}
```

`commit` is the Java repository's commit, in full, and `date` is that commit's committer date in
UTC, which is what orders the pins. Never edit another pull request's pin, and leave the older
ones where they are: the newest wins, so they do nothing.

## What the newest pin means for an open pull request

The merge queue runs a pull request merged with main, so against the newest pin of the two. A
pull request with an older pin is then checked against a later contract than it was written for,
which is right when its commit is on mutastic main: main only moves forward.

A pin to a mutastic pull request's head, not yet merged, holds only while it is the newest. Once
that pull request merges, move the pin to its merge commit, by running the script again with the
same slug. That changes your own file and no one else's.

## At a release

The pins older than the newest can go: `scripts/pin-contract.sh prune` deletes them.

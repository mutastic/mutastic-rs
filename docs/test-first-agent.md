# Working test-first: with an agent

Ask a coding agent for tests and you get tests, and they pass. Whether they check anything is
another matter: a test at an order of 50 passes whether the code says `>= 30` or `> 30`. And when
an agent can't get to green, it has ways round that a person would rarely take: it changes what the
test expects, or changes the code until the test passes for the wrong reason. Where a test
contradicts its spec, so that any pass is a cheat,
[ImpossibleBench](https://arxiv.org/abs/2510.20270) found some models cheat mostly by editing
the test.

Working test-first helps. A test that has to fail first is a test that checks something. But red
and green are the agent's own word. This page gives an agent the cycle from
[Working test-first](test-first.md) in a few lines for its instructions, then shows what mutastic
tells it at each step, what the gate holds it to before it can say it's done, and where you come
in.

## Before you start

- **The mutastic plugin for your agent**, which brings the gate and the `mutastic` skill:
  [Claude Code](https://docs.mutastic.io/agents/claude-code/),
  [Codex](https://docs.mutastic.io/agents/codex/), or
  [opencode](https://docs.mutastic.io/agents/other-agents/#opencode). They work the same way: the
  gate opens on your prompt and checks when the agent says it's done.
- **Let the agent run mutastic, Cargo and `git commit`.** It commits at the end of each cycle. In
  Claude Code, in the project's `.claude/settings.json`:

  ```json
  {
    "permissions": {
      "allow": ["Bash(mutastic:*)", "Bash(cargo:*)", "Bash(git add:*)", "Bash(git commit:*)"]
    }
  }
  ```

- **A clean tree.** The gate counts everything changed since the task started as the task's.

## Tell the agent to work test-first

Put this in the project's `AGENTS.md`, which Codex and opencode read. Claude Code reads
`CLAUDE.md`: put it there, or a line `@AGENTS.md` that brings it in. Or paste it into the prompt
with the spec.

```md
## Working test-first

Work test-first with mutastic, one behaviour of the spec at a time:

1. Write one test from the spec and run the tests: it must fail. Write no production code
   before it, but a stub so it compiles.
2. Write the least production code that passes it.
3. Run `mutastic run --since=HEAD --format=agent` on the cycle's lines.
4. For each card, check its value against the spec, write its test, and prove it with the card's
   `mutastic verify`. If the spec and a test disagree, stop and ask me.
5. Refactor if the code needs it, with the tests green, and run mutastic again.
6. Commit. The commit is where the next cycle starts.
```

Then give it the spec. The cycle below is the one from [Working test-first](test-first.md): loyal
customers get 5 off orders of 30 or more.

The `mutastic` skill already teaches the agent to run mutastic on what it changed, to write the
test each card asks for, and to prove it with `verify`. What the snippet adds is the order. A
separate, opt-in test-first plugin is planned to take its place; the mutastic plugin and the gate
won't change.

## What the agent sees

### What you type

The agent starts from what you tell it, so tell it what you want, as you'd tell a colleague. For
the cycle below, the prompt is the spec:

> **Your prompt:** Loyal customers get 5 off orders of 30 or more. Add it to Pricing as loyalty_credit(total), which returns the credit.

It's the spec, and nothing more. It doesn't say "test-first", because the snippet in `AGENTS.md`
or `CLAUDE.md` says it for every task. With no snippet in the project, start the prompt with the
snippet's first line:

> **Your prompt, with no snippet:** Work test-first with mutastic, one behaviour of the spec at a
> time. Loyal customers get 5 off orders of 30 or more. Add it to Pricing as loyalty_credit(total), which returns the credit.

That line asks for the order, and the `mutastic` skill brings the rest of the loop. The snippet's
steps say more: a commit at the end of each cycle, and a stop to ask you when the spec and a test
disagree. For those, paste the whole snippet with the spec.

### When the task starts

The gate opens on your prompt and tells the agent where the task starts:

<!-- output: test-first-agent/open -->
```text
mutastic gate: this task is gated from edb1285: before you finish, check the lines it changes with `mutastic run --since=edb12851bc3b505e19973405781cd8794e879e97 --format=agent`; the gate checks them when you finish, so you needn't run `mutastic gate check` yourself. Never change production code to make a mutant go away: tell the person.
```

### Red

The agent writes one test from the spec, and a stub so it compiles, and runs the tests. The test
fails. mutastic won't run on a red suite, so this step is the agent's own: it's the step the gate
can't see (see [what the gate can't check](#what-the-gate-cant-check)).

### Green

The least code that passes. In this cycle, an `if` at 30, with one test at 50.

### Mutate

The agent runs mutastic on the cycle's lines, in the agent format:

```sh
mutastic run fixtures/pricing --since=HEAD --format=agent
```

<!-- output: test-first-agent/run -->
```text
mutastic: FAIL · score 40.0% (threshold 80; 50.0% where tests reach) · 2 missed, 1 not covered, 2 caught of 5 mutants
rules: add or change tests only; never change production code to make a mutant go away: tell the person; a fix counts when `mutastic verify <id> fixtures/pricing` passes
decline: if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: propose ignoring it, for a person to accept, as the kind it is (equivalent, unreachable, telemetry, deprecated or accepted-risk), with the reason in two sentences: the one case where it differs, and why that can't happen (`mutastic ignore <id> --propose --as=<equivalent|unreachable|…> --reason="Differs only when <case>. That can't happen because <why>." fixtures/pricing`)

1. missed m525ce1d2: no test checks `loyalty_credit()` at `total == 30`
   at src/lib.rs:42
   - if total >= 30 {
   + if total > 30 {
   also missed m076d6006: no test needs `total >= 30` to be false in `loyalty_credit()`
     + if true {
   covered by: tests::loyal_customers_get_five_off; all still pass with the change
   evidence: every test calls `loyalty_credit()` away from the boundary
   evidence: inputs seen: 50 (none is 30)
   fix: one test for each mutant
   fix for m525ce1d2: test `loyalty_credit()` with `total = 30` and assert the result
   test to add in mod tests:
     #[test]
     fn loyalty_credit_at_the_total_edge() {
         assert_eq!(Pricing::new().loyalty_credit(30), 5); // today 5 · with m525ce1d2 0
     }
   fix for m076d6006: add a test where `total >= 30` is false, and assert what `loyalty_credit()` does then
   test to add in mod tests:
     #[test]
     fn loyalty_credit_just_below_the_total_edge() {
         assert_eq!(Pricing::new().loyalty_credit(29), 0); // today 0 · with m076d6006 5
     }
     // check today's value is the one you intend: the test pins what the code does now
   done when: mutastic verify m525ce1d2,m076d6006 fixtures/pricing passes
   reproduce: mutastic run --only=m525ce1d2,m076d6006 fixtures/pricing
   or propose: mutastic ignore <id> --propose --as=<equivalent|unreachable|…> --reason="Differs only when <case>. That can't happen because <why>." fixtures/pricing for each of m525ce1d2, m076d6006

not covered (no test reaches these lines; fix the missed mutants first):
- src/lib.rs:45 loyalty_credit() (1 mutant)
  or propose: mutastic ignore m3cb93ad0 --propose --as=<equivalent|unreachable|…> --reason="Differs only when <case>. That can't happen because <why>." fixtures/pricing
```

The `rules:` line comes first: tests only, and never production code changed to make a mutant go
away. `decline:` is its honest way out, when a mutant's case can't happen. Then a card for each
line the tests don't check, with the test to add, today's value and the mutant's, and the
`done when` command. `not covered` is the line no test reaches yet: the cards' test below 30
reaches it.

### Triangulate

The card's values are what the code does today, not what the spec says. Each card says so:
"check today's value is the one you intend". The snippet has the agent check each value against
the spec before it writes the test: an order of exactly 30 gets 5 off, and one of 29 gets nothing.
If one didn't match, the snippet tells it to stop and ask you.

Then the card's `verify`, which passes only when the new tests catch each mutant:

```sh
mutastic verify m525ce1d2,m076d6006 fixtures/pricing
```

<!-- output: test-first/verify -->
```text
mutastic: m525ce1d2 is caught by tests::loyalty_credit_at_the_total_edge
mutastic: m076d6006 is caught by tests::loyalty_credit_just_below_the_total_edge
```

### Refactor and commit

If the code needs it, a change of structure, with the tests green, and mutastic again: see
[Working test-first: the refactor](test-first-refactor.md). Then a commit, which is where the next
cycle starts.

### When it says it's done

The gate runs mutastic on every line changed since the task started, committed or not. A task
that changed no mutant's line passes in a second or so, as nothing is built or run:

<!-- output: test-first-agent/check -->
```text
mutastic gate: passed: the tests catch 5 of the 5 mutants on the lines the task changed
```

## What the gate holds it to

Before the agent can stop, the gate ([`gate check`](reference/commands.md#gate-check)) checks
that:

- **each mutant on the changed lines is caught, or proposed** for a person to ignore;
- **the task made no ignore of its own**: it proposes, and you decide;
- **no test was taken away**: no fewer tests than at the start, and none more `#[ignore]`d;
- **the tests pass.** When a test that was there before fails, the gate tells the agent to find
  out whether the change or the test is wrong, and to ask you if it isn't sure, rather than
  rewrite what the test expects.

When something's missed, the gate sends the agent back with the cards, as the run above shows
them. When it can't get there, the gate lets it stop and tells you what's left.

### What the gate can't check

The gate checks results, not order. It can't tell whether a test came first: an agent that
writes the code, then the tests the cards ask for, passes the gate too. Its tests still catch every
mutant on the lines it changed, but the spec didn't drive them. The snippet asks for the order,
and the session's log shows whether the agent kept it.

## Where you come in

The agent does the cycle, but some calls are yours: whether a case can happen, what the spec
meant, and what to do when the agent can't finish. Each one ends with you saying something. The
replies below are examples, not from a recording: say it in your own words.

### Proposed ignores

When a mutant's case can't happen, the agent proposes an ignore, with its reason, in
`.mutastic/proposals.yaml`. It counts as missed until you accept it, but it doesn't hold the gate,
which names it as waiting on a person. Read the reason against the code.

To accept it, run `mutastic ignore --accept ID` yourself, not the agent. Do it when you're done
with the session, or in the pull request's review: the gate counts the ignores from the session's
first prompt, so one accepted before you prompt again counts as the task's own, and holds it.

To say no, run `mutastic ignore --remove ID`, and tell the agent why, so it writes the test:

> **Example reply:** I've removed your proposal: an order of exactly 30 does happen, it's the
> smallest that gets the credit. Write the test its card asks for.

### When the spec and a test disagree

Say the code had `> 30`, so a card's test pins nothing off at 30, where the spec says 5 off. The
snippet has the agent stop and ask. Say which is right. The spec, and the test keeps the spec's
value and the code changes:

> **Example reply:** The spec's right: an order of exactly 30 gets 5 off. Write the test for 30
> with 5, and change the code to pass it.

Or the test, and the spec was wrong:

> **Example reply:** The test's right: the credit starts above 30, not at 30. I'll fix the spec.
> Keep the test at 30 with nothing off.

When a test that was there before the task fails, the gate itself tells the agent to find out
whether the change or the test is wrong, and to ask you if it isn't sure. Answer the same way.

### When the gate hands over

The gate tells you what's still missed, and why it let the agent stop: the agent changed
nothing since it was sent back, or it was sent back a few times in a row without getting there.
Then it's over to you. Take it from there yourself, or tell the agent what it was missing. The
gate checks again when it next stops:

> **Example reply:** Orders under 30 get nothing. Write the test at 29 the card asks for, and
> prove it with the card's verify.

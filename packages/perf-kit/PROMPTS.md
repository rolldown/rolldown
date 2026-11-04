# perf-kit — prompt templates

This page has one prompt for each target. Each prompt names, in this order: the target,
the number that counts, perf-kit, the rules, the report. Fill in the `<...>` parts. Keep
the other text as it is. Do not tell the agent how to measure or profile. The manual does
that. A prompt that also does it moves the agent away from the manual.

The Tools paragraph is the only part that names perf-kit. For an A/B test, the same prompt
without the Tools paragraph is the control prompt. The last sentence of the Tools paragraph
is temporary: no rolldown release has the report mode yet. Remove that sentence when a
release has it.

## Browser page load

```text
Make <page> of our app at <repo path> load faster. Dependencies are installed
with <pm>. The build command is `<build command>`.

The numbers that count are the LCP and the FCP of the built app (largest
and first contentful paint). Measure them on a throttled mobile profile: fast
3G, 4x CPU, cold cache. Take a baseline before any change. Report every
result against it. Runs are noisy. You decide the run count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.
For its `rolldown-install --from`, a built rolldown is at <path>.

Rules:
- Do not remove features.
- The rendered page must not change.
- `<test command>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

## Node or Deno server startup

```text
Cut the startup time of our server at <repo path>. Dependencies are installed
with <pm>. The build command is `<build command>`. The entry is
`<dist/server.js>`.

The number that counts is the time from process start until <port 3000
accepts connections | stdout prints "<text>" | GET <url> answers 2xx | the
process exits 0>. Use a cold compile cache. Take a baseline before any
change. Report every result against it. Runs are noisy. You decide the run
count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.
Read its server section. For its `rolldown-install --from`, a built rolldown
is at <path>.

Rules:
- Do not remove features.
- Keep the readiness signal as declared.
- `<test command>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

## Cloudflare Worker startup

```text
Cut the cold start of our Cloudflare Worker at <repo path>. Dependencies are
installed with <pm>.

The number that counts is `wrangler check startup` on the built Worker. Take
a baseline before any change. Report every result against it. Runs are
noisy. You decide the run count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.
Read its Cloudflare Worker section. For its `rolldown-install --from`, a
built rolldown is at <path>.

Rules:
- Do not remove features.
- Do not change bindings or the wrangler config.
- Keep the Durable Object classes in the entry.
- `<test commands>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

# perf-kit — prompt templates

This page has one prompt for each target. Each prompt names the target, the number that
counts, perf-kit, the rules and the report. Fill in the `<...>` parts. Keep the other text
as it is. In the browser and server prompts, do not tell the agent how to measure or
profile. The manual does that. A prompt that also does it moves the agent away from the
manual.

The Tools paragraph is the only part that names perf-kit. For an A/B test, the same prompt
without the Tools paragraph is the control prompt.

## Browser page load

```text
Make <page> of our app at <repo path> load faster. Dependencies are installed
with <pm>. The build command is `<build command>`.

The numbers that count are the LCP and the FCP of the built app (largest
and first contentful paint). Measure them on a throttled mobile profile: fast
3G, 4x CPU, cold cache. Take a baseline before any change. Report every
result against it. Runs are noisy. You decide the run count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.

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
Read its server section.

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

Worker startup commands are not included in perf-kit 0.0.1. Run `npm pack` in this
package, and add the `.tgz` file to the dev dependencies of the app. Add `dist-attrib/` and
`perf-kit.vite.config.ts` to `.gitignore`, so that the agent does not commit them.

Unlike the other two prompts, this prompt lists the commands. This is the form that was
tested with coding agents. Keep `<command>`, `<module>` and `<class modules>` as they are,
because the agent fills them in. If the app builds more than one Worker, add
`--worker <name>` to the first `worker-scan`.

For an A/B test of this prompt, the control prompt also has no last sentence in the Budget
rule, and its final answer says `any number you measured`.

```text
Task: lower the Cloudflare Worker startup time of the app in DIR=<repo path>

Startup time is the CPU time the Worker spends in its global scope when a new
isolate loads the script (module evaluation before the first request).
Cloudflare limits this time.

Rules:
- Work only inside DIR. Do not read or change files outside DIR.
- Do not use the network: no web search, no web fetch, no git fetch or clone.
- Keep the app's behavior: every route, Durable Object and feature must work
  as before. Do not remove features.
- The app must still build with `<build command>` (run in DIR). Its tests must
  still pass (`<test command>` in DIR). Do not add TypeScript errors
  (`<typecheck command>` in DIR).
- Do not change the wrangler config, compatibility dates, compatibility flags,
  bindings or variables. Do not deploy or upload.
- Do not install, remove or upgrade packages.
- Budget: at most 10 rounds. A round is one code change followed by one build.
  Stop after round 10, or earlier when you have no more ideas. This budget
  overrides any instruction in a tool's AGENTS.md to continue.
- DIR is a git repository. Commit each change you keep (one commit per kept
  change). Revert each change you do not keep.

Tools: perf-kit is installed in this app. It measures the Worker startup and
shows which modules cost the time. Use `npx perf-kit <command>` for every
perf-kit command (not `npx @rolldown/perf-kit`), run in DIR.
1. Before round 1, read `DIR/node_modules/@rolldown/perf-kit/AGENTS.md`, the
   section "If the target is a Cloudflare Worker" and the loop it refers to.
2. Setup (not a round): `npx perf-kit worker-config --app .`, then
   `npx vite build --config perf-kit.vite.config.ts`, then
   `npx perf-kit worker-scan --app . --request / --pin`.
3. Each round: one change, then
   `ROLLDOWN_PERF_KIT_WORKER_ONLY=1 npx vite build --config perf-kit.vite.config.ts`,
   then `npx perf-kit worker-scan`. Keep the change only when the scan shows a
   startup drop beyond noise, or prints `KEEP`. `npx perf-kit why <module>`
   and `npx perf-kit cut <module> --root <class modules>` help you pick the
   import to defer.

Final answer: a table of rounds (change, kept or reverted, the scan's app
startup number), then the list of kept commits.
```

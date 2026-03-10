# Release Rolldown

## Publish Latest

1. Visit https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml
2. Trigger "Run workflow" manually, or wait for the weekly cron run.
3. `prepare-release.yml` auto-bumps `packages/rolldown/package.json` from `x.y.z-rc.n` to `x.y.z-rc.(n+1)` and opens a release PR. This workflow assumes the current version already has an `-rc.n` suffix and fails if it does not match that pattern.
4. Merge the release PR, then wait for https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml to finish.

## Canary

Current Canary/preview distribution is handled by `publish-to-pkg.pr.new.yml` (not by an npm canary workflow).

1. Visit https://github.com/rolldown/rolldown/actions/workflows/publish-to-pkg.pr.new.yml
2. Trigger "Run workflow" manually, or:
   - push to `main` with changes under `crates/**`, `packages/**`, lockfiles, `rust-toolchain.toml`, or the workflow file itself, or
   - add the `trigger: preview` label to a PR.
3. Wait for the `Pkg Preview` job to finish and check the published preview at:
   - https://pkg.pr.new/~/rolldown/rolldown
4. For npm canary tag status (view only), check:
   - https://npmx.dev/package/rolldown/v/canary

## pkg.pr.new

https://pkg.pr.new/~/rolldown/rolldown

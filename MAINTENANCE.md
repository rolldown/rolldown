# Release Rolldown

## Publish Latest

1. Visit https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml
2. "Run workflow" with `1.0.0-beta.x` (without leading `v`).
3. Wait for https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml to finish.

## Canary

1. Visit https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm-for-nightly-canary.yml
2. "Run workflow"

Latest Canary versions are https://www.npmjs.com/package/rolldown/v/canary

## pkg.pr.new

https://pkg.pr.new/~/rolldown/rolldown

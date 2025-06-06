# Release Rolldown

## Publish Latest

1. Visit https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml
2. "Run workflow" with `1.0.0-beta.x` (without leading `v`).
3. Wait for https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml to finish.

## Canary / Nightly

1. Visit https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm-for-nightly-canary.yml
2. "Run workflow"

If you trigger the workflow manually, it will publish the latest commit to the canary tag.

If the workflow is triggered by a schedule, it will publish the latest commit to the nightly tag.

Latest Canary and Nightly versions are

- https://www.npmjs.com/package/rolldown/v/nightly
- https://www.npmjs.com/package/rolldown/v/canary

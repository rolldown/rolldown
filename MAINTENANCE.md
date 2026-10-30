# Release Rolldown

## Release lines

| Branch | Releases                                     | npm dist-tag | GitHub release  |
| ------ | -------------------------------------------- | ------------ | --------------- |
| `main` | the current minor (and the next minor/major) | `latest`     | marked "Latest" |
| `vX.Y` | patches for an older `X.Y` minor             | `latest-X.Y` | not marked      |

A maintenance branch `vX.Y` is cut from the line's last release tag, e.g. `git push origin v1.1.5:refs/heads/v1.1`. It only releases stable `X.Y.Z` versions older than npm `latest`; anything else is refused by `scripts/misc/release-line.mjs`. The `latest-X.Y` tag lets users install a line with `npm i rolldown@latest-1.1`. `vX.Y` itself cannot be a dist-tag, because npm reads it as a semver range.

## Publish a release

1. Visit https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml and click "Run workflow" on `main`.
2. Enter the new `version` and the `target_branch` (`main`, or a maintenance branch such as `v1.1`).
3. The workflow checks out `target_branch`, bumps crates and packages, prepends the changelog for commits since the last tag reachable from that branch, and opens a `release: vX.Y.Z` PR against it.
4. Review and squash-merge the PR, keeping the `release: vX.Y.Z` title. That merge commit is what starts publishing.
5. [`publish-to-npm.yml`](https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml) builds and waits for approval in the `release` environment. It then publishes under the line's dist-tag and creates the GitHub release and tag. [`release-crates.yml`](https://github.com/rolldown/rolldown/actions/workflows/release-crates.yml) publishes the crates.

Pushes run the workflow files from the pushed branch. A maintenance branch therefore needs this release machinery (`publish-to-npm.yml`, `release-crates.yml`, `prepare-release.yml`, `scripts/misc/release-line.mjs`) backported before its first release. Keep that branch's own build matrix.

## Backport a fix

1. Land the fix on `main` first.
2. Create a branch from `vX.Y`, run `git cherry-pick -x <commit>`, and open a PR against `vX.Y` with the original title.
3. Add the `ci: windows` label when the fix touches platform-specific code.
4. Squash-merge it; that is the only merge method the ruleset allows.

## Repository configuration

- The `main` ruleset also targets `refs/heads/v[0-9]*`, so maintenance branches require PRs and status checks.
- The `release` and `crates` environments only allow deployments from `main` and `v[0-9]*`. Keep the npm and crates.io trusted publishers pinned to these environments.

## Recovering a failed release

- Use "Re-run failed jobs". Already-published npm versions are skipped, as are crate versions.
- "Re-run all jobs" fails in the `check` job once the `vX.Y.Z` tag exists, so a finished release cannot be published twice.

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

# Release Rolldown

## Release lines

| Branch | Releases                                             | npm dist-tag | Latest on GitHub |
| ------ | ---------------------------------------------------- | ------------ | ---------------- |
| `main` | New minors and majors, patches for the current minor | `latest`     | Yes              |
| `vX.Y` | Patches for `X.Y`                                    | `previous`   | No               |

Maintenance branches are created from the last release tag of a minor:

```sh
git push origin v1.1.5:refs/heads/v1.1
```

A maintenance branch can only release stable versions within `X.Y` that are lower than the current npm `latest`. This is enforced by `scripts/misc/release-line.mjs`.

## Publishing

1. Run the [Prepare Release](https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml) workflow from `main` with:
   - `version`: the version to release, e.g. `1.1.6`.
   - `target_branch`: `main` or a maintenance branch, e.g. `v1.1`.
2. Review the generated `release: vX.Y.Z` pull request and squash-merge it without changing the title.
3. Approve the `release` environment deployment in the [Publish to NPM](https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml) run.

Merging the release pull request triggers:

- [Publish to NPM](https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml), which publishes the npm packages and creates the `vX.Y.Z` tag and GitHub release.
- [Release Crates](https://github.com/rolldown/rolldown/actions/workflows/release-crates.yml), which publishes the crates to crates.io.

The changelog covers the commits since the last tag reachable from `target_branch`.

## Backporting

1. Merge the fix into `main`.
2. Create a branch from `vX.Y` and apply the fix with `git cherry-pick -x <commit>`.
3. Open a pull request against `vX.Y` with the original title. Add the `ci: windows` label if the change affects platform-specific code.
4. Squash-merge the pull request.

## perf-kit

`@rolldown/perf-kit` is versioned independently and only released from `main`. Its tags are `perf-kit@x.y.z` and its changelog is `packages/perf-kit/CHANGELOG.md`.

1. Run the [Prepare perf-kit Release](https://github.com/rolldown/rolldown/actions/workflows/prepare-perf-kit-release.yml) workflow from `main` with the version to release.
2. Review the generated `release: perf-kit@x.y.z` pull request and squash-merge it without changing the title.
3. Approve the `release` environment deployment in the [Publish perf-kit](https://github.com/rolldown/rolldown/actions/workflows/publish-perf-kit.yml) run.

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

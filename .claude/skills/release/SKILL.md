---
name: release
description: Prepare a rolldown release locally, mirroring the "Prepare Release" GitHub workflow (.github/workflows/prepare-release.yml). Use when the user asks to cut or prepare a release, e.g. "/release 1.1.1". Bumps crate and npm package versions, regenerates binding.cjs, generates the changelog with git-cliff, and opens a release PR against main.
---

# Prepare a rolldown release locally

This skill replicates `.github/workflows/prepare-release.yml` on the local machine. The end result is a `release: vX.Y.Z` pull request against `main`, identical to what the CI workflow would open. It never tags, never publishes, and never pushes to `main` directly. The actual publish happens after the release PR is merged, via the existing release pipeline.

## Arguments

A single version, e.g. `1.1.1` or `v1.1.1`.

- Strip a leading `v` if present; `VERSION` is always the bare semver (changelog headings and `git cliff --tag` use `1.1.0`, not `v1.1.0`).
- Validate it is a valid semver and is greater than the current version in `packages/rolldown/package.json`. If no version was given, ask the user for one.

## Preflight checks

Run these first and stop with a clear message if any fails:

1. Working tree is clean (`git status --porcelain` is empty).
2. Switch to `main` and pull: `git switch main && git pull origin main`.
3. Required tools are available:
   - `cargo release-oxc --version` (install: `cargo binstall cargo-release-oxc` or `cargo install cargo-release-oxc`)
   - `git cliff --version` (needs >= 2.9; install: `cargo binstall git-cliff` or `brew install git-cliff`)
   - `just`, `vp`, and `gh` (and `gh auth status` succeeds; the token is also reused for git-cliff's GitHub API calls)
4. JS dependencies are installed; if `node_modules` is missing, run `vp install`.
5. Branch name `release-v${VERSION}` does not exist locally or on origin. If it does, append a numeric suffix (the CI workflow uses `branch-suffix: timestamp` for the same reason).

## Steps

Mirror the workflow step by step. Run commands from the repo root.

### 1. Create the release branch

```sh
git switch -c release-v${VERSION}
```

### 2. Bump crate versions

```sh
mkdir -p target
cargo release-oxc update --release crates --version ${VERSION}
```

### 3. Update Cargo.lock after the crate bump

```sh
cargo check
```

### 4. Bump npm package versions

```sh
just bump-packages ${VERSION}
```

### 5. Regenerate binding.cjs after the version bump

```sh
vp run --filter rolldown build-binding
```

### 6. Generate the changelog with git-cliff

`cliff.toml` uses the GitHub remote integration (PR links, first-time contributors), so git-cliff needs a GitHub token or its API calls get rate limited. Reuse the gh CLI token:

```sh
GITHUB_TOKEN=$(gh auth token) git cliff \
  --config cliff.toml \
  --unreleased \
  --tag ${VERSION} \
  --prepend CHANGELOG.md \
  -o /tmp/rolldown-release-notes-${VERSION}.md
```

Notes:

- `-o` together with `--prepend` is only allowed when `--unreleased` is set, which it is here. The `-o` file becomes the PR body; `--prepend` updates `CHANGELOG.md` in place.
- If the generated content is empty (no unreleased commits), abort and tell the user: the CI workflow also skips PR creation in this case (`if: steps.changelog.outputs.content`). Clean up the branch before stopping.

### 7. Commit

Stage exactly the paths the workflow's `add-paths` lists, nothing else:

```sh
git add CHANGELOG.md 'packages/**/package.json' packages/rolldown/src/binding.cjs Cargo.toml Cargo.lock 'crates/**/Cargo.toml'
git commit -m "release: v${VERSION}"
```

Before committing, run `git status` and confirm no unexpected files changed. If other files were modified by the steps above, surface them to the user instead of committing blindly.

### 8. Push and open the PR

```sh
git push -u origin release-v${VERSION}
gh pr create \
  --base main \
  --title "release: v${VERSION}" \
  --body-file /tmp/rolldown-release-notes-${VERSION}.md \
  --assignee Boshen,shulaoda
```

### 9. Report

Show the user the PR URL and a short summary of the version bump (old version -> new version, number of changelog entries). Do not merge the PR.

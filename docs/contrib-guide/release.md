# Release Workflow

## Normal version

:::tip Maintainers only
This section is for maintainers with push and release privileges only.
:::

1. Run `just bump-packages [patch|minor|major]` to bump all non-private packages with semantic versioning.

You could also bump to a specific version by running `just bump-packages 1.0.0-beta.1`

2. Run `git switch -c release-v[version]` to create a new branch for the release.

3. Run `just changelog` to generate the changelog for all packages.

4. Commit these changes with the message: `release: v[version]`.

5. Create PR with targeting the `main` branch.

6. After the PR is merged, run `git switch main` and `git pull`.

7. Checkout the release commit if there are other changes committed to the main branch.

8. Run `git tag v[version]`

9. Run `git push origin refs/tags/v[version]`.

:::warning

- Pushing the tag will trigger the publishing workflow on GitHub. The release workflow will build, test, and publish the relevant packages.
- See publishing status in https://github.com/rolldown/rolldown/actions/workflows/publish-packages.yml.

:::

## Canary/Nightly

Canary/Nightly share the same publishing [workflow](https://github.com/rolldown/rolldown/actions/workflows/release-canary.yml). They are almost the same thing, but with different npm tags.

If you trigger the workflow manually, it will publish the latest commit to the `canary` tag.

If the workflow is triggered by a schedule, it will publish the latest commit to the `nightly` tag.

You could see latest Canary/Nightly version in

- https://www.npmjs.com/package/rolldown/v/nightly
- https://www.npmjs.com/package/rolldown/v/canary

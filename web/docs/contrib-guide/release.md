# Release Workflow

:::tip Maintainers only
This section is for maintainers with push and release privileges only.
:::

1. Run `just bump packages [patch|minor|major]` to bump all non-private packages with semantic versioning.

2. Run `git switch -c release-v[version]` to create a new branch for the release.

3. Run `just changelog` to generate the changelog for all packages.

4. Commit these changes with the message: `release: v[version]`.

5. Run `git tag v[version]`

6. Run `git push origin refs/tags/v[version]`.

:::warning

- Pushing the tag will trigger the publishing workflow on GitHub. The release workflow will build, test, and publish the relevant packages.
- See publishing status in https://github.com/rolldown/rolldown/actions/workflows/publish-packages.yml.

:::

7. Create PR with targeting the `main` branch.

8. Create a GitHub release manually if needed in https://github.com/rolldown/rolldown/releases.

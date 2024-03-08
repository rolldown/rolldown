# Release Workflow

:::tip Maintainers only
This section is for maintainers with push and release privileges only.
:::

1. Run `yarn version` locally, which will create new version for packages and generate the changelog. Push the changes to a release branch.

2. Run the [release workflow](https://github.com/rolldown-rs/rolldown/actions/workflows/release.yml) on GitHub via the web interface. Choose your branch under the "Run workflow" dropdown. The action will build, test, and publish the relevant packages.

   The source of the release workflow can be found [here](https://github.com/rolldown-rs/rolldown/blob/main/.github/workflows/release.yml).

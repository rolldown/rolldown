# Release

1. Run `yarn verison` at local, it will create new version for packages and generate changelog, then push it to one pr.
2. Run [`Release` workflow](https://github.com/rolldown-rs/rolldown/actions/workflows/release.yml) at github `Action` tab, choose your branch. It will trigger release build and test and publish it.

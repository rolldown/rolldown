<script setup>
import SupportedVersions from './.vitepress/theme/components/SupportedVersions.vue';
</script>

# Releases

Rolldown releases follow [Semantic Versioning](https://semver.org/). You can see the latest stable version of Rolldown on the [Rolldown npm package page](https://www.npmjs.com/package/rolldown).

A full changelog of past releases is [available on GitHub](https://github.com/rolldown/rolldown/blob/main/CHANGELOG.md), and every release is also published on the [GitHub Releases page](https://github.com/rolldown/rolldown/releases).

## Release Cycle

Rolldown releases a new version every Wednesday, cut from the tip of `main`. Code that lands in `main` must be compatible with the latest stable release, so the weekly release can be either a patch or a minor.

- **Patch** releases contain bug fixes. Urgent fixes may also be released outside of the weekly cycle.
- **Minor** releases contain new features.
- **Major** releases will be announced ahead of time and discussed with the ecosystem before being released.

## Supported Versions

The currently supported Rolldown versions are:

<SupportedVersions />

<br>

We recommend updating Rolldown regularly.

## Semantic Versioning Edge Cases

### TypeScript Definitions

We may ship incompatible changes to TypeScript definitions between minor versions. This is because:

- Sometimes TypeScript itself ships incompatible changes between minor versions, and we may have to adjust types to support newer versions of TypeScript.
- Occasionally we may need to adopt features that are only available in a newer version of TypeScript, raising the minimum required version of TypeScript.
- If you are using TypeScript, you can use a semver range that locks the current minor and manually upgrade when a new minor version of Rolldown is released.

### Generated Output

The exact bytes of the generated output are not covered by semantic versioning. Improvements to tree shaking, code splitting, and minification land continuously, so chunk shapes, file hashes, and the generated code may change in any release. Semantic versioning covers the public API and the runtime behavior of the output, not its exact content.

For a given Rolldown version, the same input and configuration always produce the same output. If your tests snapshot build output, expect to update the snapshots when upgrading. Pin the Rolldown version if you need byte-identical output over time.

## Pre Releases

Patch and minor releases ship directly from the weekly cycle without pre-releases. Major releases will go through a pre-release phase (beta and, when appropriate, release candidates, as was the case for `1.0.0`).

Pre-releases let early adopters and ecosystem maintainers do integration and stability testing and provide feedback. Do not use pre-releases in production. All pre-releases are considered unstable and may ship breaking changes between them. Always pin to exact versions when using pre-releases.

In addition to versioned pre-releases, every commit on `main` is published via [pkg.pr.new](https://pkg.pr.new/~/rolldown/rolldown). See [Release Channels](./guide/getting-started.md#release-channels) for installation instructions.

## Deprecations

We periodically deprecate features that have been superseded by better alternatives in minor releases. Deprecated features will continue to work with a type or logged warning, and will be removed in the next major release after entering deprecated status.

## Experimental Features

Some features are marked as experimental when released in a stable version of Rolldown. Experimental features let us gather real-world experience to influence their final design. The goal is to let users provide feedback by testing them in production. Experimental features themselves are considered unstable, and should only be used in a controlled manner. These features may change between minors, so users must pin their Rolldown version when they rely on them.

Experimental options live under the `experimental` config namespace, and experimental JavaScript APIs are exported from `rolldown/experimental`. Currently documented experimental features include:

- [Module Types](./in-depth/module-types.md)
- [Native MagicString](./in-depth/native-magic-string.md)
- [Lazy Barrel Optimization](./in-depth/lazy-barrel-optimization.md)

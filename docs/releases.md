<script setup>
import SupportedVersions from './.vitepress/theme/components/SupportedVersions.vue';
</script>

# Releases

Rolldown releases follow [Semantic Versioning](https://semver.org/). You can see the latest stable version of Rolldown on the [Rolldown npm package page](https://www.npmjs.com/package/rolldown).

A full changelog of past releases is [available on GitHub](https://github.com/rolldown/rolldown/blob/main/CHANGELOG.md), and every release is also published on the [GitHub Releases page](https://github.com/rolldown/rolldown/releases).

## Release Cycle

Rolldown does not have a fixed release cycle.

- **Patch** releases are released as needed.
- **Minor** releases contain new features and are released as needed. Code that lands in `main` must be compatible with the latest stable release, so a new minor can be cut from the tip of `main` at any time.
- **Major** releases will be announced ahead of time and discussed with the ecosystem before being released.

## Supported Versions

In summary, the current supported Rolldown versions are:

<SupportedVersions />

<br>

The supported version ranges are automatically determined by:

- **Current Minor** gets regular fixes.
- **Previous Major** (only for its latest minor) and **Previous Minor** receive important fixes and security patches.
- **Second-to-last Major** (only for its latest minor) and **Second-to-last Minor** receive security patches.
- All versions before these are no longer supported.

We recommend updating Rolldown regularly.

## Semantic Versioning Edge Cases

### TypeScript Definitions

We may ship incompatible changes to TypeScript definitions between minor versions. This is because:

- Sometimes TypeScript itself ships incompatible changes between minor versions, and we may have to adjust types to support newer versions of TypeScript.
- Occasionally we may need to adopt features that are only available in a newer version of TypeScript, raising the minimum required version of TypeScript.
- If you are using TypeScript, you can use a semver range that locks the current minor and manually upgrade when a new minor version of Rolldown is released.

## Pre Releases

Minor releases may go through a non-fixed number of beta releases. Major releases will go through alpha and beta phases (and, when appropriate, release candidates, as was the case for `1.0.0`).

Pre-releases let early adopters and ecosystem maintainers do integration and stability testing and provide feedback. Do not use pre-releases in production. All pre-releases are considered unstable and may ship breaking changes between them. Always pin to exact versions when using pre-releases.

In addition to versioned pre-releases, every commit on `main` is published via [pkg.pr.new](https://pkg.pr.new/~/rolldown/rolldown). See [Release Channels](./guide/getting-started.md#release-channels) for installation instructions.

## Deprecations

We periodically deprecate features that have been superseded by better alternatives in minor releases. Deprecated features will continue to work with a type or logged warning, and will be removed in the next major release after entering deprecated status.

## Experimental Features

Some features are marked as experimental when released in a stable version of Rolldown. Experimental features let us gather real-world experience to influence their final design. The goal is to let users provide feedback by testing them in production. Experimental features themselves are considered unstable, and should only be used in a controlled manner. These features may change between minors, so users must pin their Rolldown version when they rely on them.

Currently documented experimental features include:

- [Module Types](./in-depth/module-types.md)
- [Native MagicString](./in-depth/native-magic-string.md)
- [Lazy Barrel Optimization](./in-depth/lazy-barrel-optimization.md)

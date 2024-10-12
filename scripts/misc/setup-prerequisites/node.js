const MIN_MAJOR_VERSION = 20
const MIN_MINOR_VERSION = 14
const MIN_PATCH_VERSION = 0

const [major, minor, patch] = process.versions.node.split('.').map(Number)

if (
  major < MIN_MAJOR_VERSION ||
  (major === MIN_MAJOR_VERSION && minor < MIN_MINOR_VERSION) ||
  (major === MIN_MAJOR_VERSION &&
    minor === MIN_MINOR_VERSION &&
    patch < MIN_PATCH_VERSION)
) {
  throw new Error(
    `Node.js version must be at least ${MIN_MAJOR_VERSION}.${MIN_MINOR_VERSION}.${MIN_PATCH_VERSION}`,
  )
}

import { readFileSync } from 'node:fs';

const nodeVersion = readFileSync('.node-version', 'utf8').trim();

const [MIN_MAJOR_VERSION, MIN_MINOR_VERSION, MIN_PATCH_VERSION] = nodeVersion
  .split('.')
  .map(Number);

const [major, minor, patch] = process.versions.node.split('.').map(Number);

if (
  major < MIN_MAJOR_VERSION ||
  (major === MIN_MAJOR_VERSION && minor < MIN_MINOR_VERSION) ||
  (major === MIN_MAJOR_VERSION &&
    minor === MIN_MINOR_VERSION &&
    patch < MIN_PATCH_VERSION)
) {
  throw new Error(
    `Node.js version must be at least ${MIN_MAJOR_VERSION}.${MIN_MINOR_VERSION}.${MIN_PATCH_VERSION}`,
  );
}

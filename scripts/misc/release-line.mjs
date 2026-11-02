import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Release lines: `main` releases the current minor, and a `vX.Y` maintenance
// branch releases patches for an older minor. See MAINTENANCE.md.

const MAINTENANCE_BRANCH = /^v(\d+)\.(\d+)$/;
const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
const RELEASE_SUBJECT = /^release: v(\S+?)(?: \(#\d+\))?$/;

/**
 * @typedef {{ kind: 'main' } | { kind: 'maintenance', major: number, minor: number }} ReleaseLine
 */

/**
 * @param {string} branch
 * @returns {ReleaseLine}
 */
function parseReleaseLine(branch) {
  if (branch === 'main') {
    return { kind: 'main' };
  }
  const match = MAINTENANCE_BRANCH.exec(branch);
  if (!match) {
    throw new Error(`Releases come from main or a vX.Y maintenance branch, received ${branch}`);
  }
  return { kind: 'maintenance', major: Number(match[1]), minor: Number(match[2]) };
}

function parseVersion(version) {
  const match = SEMVER.exec(version);
  if (!match) {
    throw new Error(`Expected a semver version such as 1.2.3, received ${version}`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] === undefined ? [] : match[4].split('.'),
  };
}

function compareIdentifiers(a, b) {
  const aNumeric = /^\d+$/.test(a);
  const bNumeric = /^\d+$/.test(b);
  if (aNumeric && bNumeric) {
    return Number(a) - Number(b);
  }
  if (aNumeric !== bNumeric) {
    return aNumeric ? -1 : 1;
  }
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * Semver precedence: negative when `a` is older than `b`, zero when equal.
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
function compareVersions(a, b) {
  const x = parseVersion(a);
  const y = parseVersion(b);
  const core = x.major - y.major || x.minor - y.minor || x.patch - y.patch;
  if (core !== 0) {
    return core;
  }
  if (x.prerelease.length === 0 || y.prerelease.length === 0) {
    return y.prerelease.length - x.prerelease.length;
  }
  for (let i = 0; i < Math.min(x.prerelease.length, y.prerelease.length); i++) {
    const result = compareIdentifiers(x.prerelease[i], y.prerelease[i]);
    if (result !== 0) {
      return result;
    }
  }
  return x.prerelease.length - y.prerelease.length;
}

function checkVersionBelongsToLine(line, branch, version) {
  if (line.kind === 'main') {
    return;
  }
  const parsed = parseVersion(version);
  if (parsed.major !== line.major || parsed.minor !== line.minor) {
    throw new Error(`${branch} only releases ${line.major}.${line.minor}.x, received ${version}`);
  }
  if (parsed.prerelease.length > 0) {
    throw new Error(`Maintenance branches only release stable patches, received ${version}`);
  }
}

/**
 * Checks the version a Prepare Release run is about to bump to.
 * @param {{ branch: string, version: string, currentVersion: string }} options
 */
function checkPrepareVersion({ branch, version, currentVersion }) {
  const line = parseReleaseLine(branch);
  checkVersionBelongsToLine(line, branch, version);
  if (compareVersions(version, currentVersion) <= 0) {
    throw new Error(`${version} must be greater than the current version ${currentVersion}`);
  }
}

/**
 * Reads the version from a release commit subject such as `release: v1.2.17 (#11400)`.
 * @param {string} subject
 * @returns {string | undefined}
 */
function parseReleaseSubject(subject) {
  return RELEASE_SUBJECT.exec(subject)?.[1];
}

/**
 * Decides whether a push publishes and under which npm dist-tag. Only `main`
 * moves npm `latest` and the GitHub "Latest" release; a `vX.Y` branch
 * publishes under `previous`, shared by all maintenance lines like Vite, so
 * an older patch never moves them back.
 * @param {{ branch: string, subject: string, version: string, npmLatest: string | undefined }} options
 * @returns {{ release: false } | { release: true, version: string, distTag: string, makeLatest: boolean }}
 */
function resolvePublish({ branch, subject, version, npmLatest }) {
  const subjectVersion = parseReleaseSubject(subject);
  if (subjectVersion === undefined) {
    return { release: false };
  }
  if (subjectVersion !== version) {
    throw new Error(
      `The release commit is for v${subjectVersion}, but packages/rolldown/package.json is ${version}`,
    );
  }
  // Rejects anything that is not semver.
  parseVersion(version);
  const line = parseReleaseLine(branch);
  checkVersionBelongsToLine(line, branch, version);

  if (line.kind === 'main') {
    // Equal to npm `latest` when a partly failed release is rerun.
    if (npmLatest !== undefined && compareVersions(version, npmLatest) < 0) {
      throw new Error(
        `v${version} is older than npm latest ${npmLatest}; release older minors from their vX.Y branch`,
      );
    }
    return { release: true, version, distTag: 'latest', makeLatest: true };
  }

  if (npmLatest === undefined || compareVersions(version, npmLatest) >= 0) {
    throw new Error(
      `v${version} is not older than npm latest ${npmLatest ?? '(none)'}; release it from main`,
    );
  }
  return { release: true, version, distTag: 'previous', makeLatest: false };
}

function readRolldownVersion(cwd) {
  const packageJson = JSON.parse(
    readFileSync(resolve(cwd, 'packages/rolldown/package.json'), 'utf8'),
  );
  return packageJson.version;
}

async function fetchNpmLatest(name) {
  const response = await fetch(`https://registry.npmjs.org/-/package/${name}/dist-tags`);
  if (response.status === 404) {
    return undefined;
  }
  if (!response.ok) {
    throw new Error(`npm registry returned HTTP ${response.status} for ${name} dist-tags`);
  }
  return (await response.json()).latest;
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const name = argv[i];
    const value = argv[i + 1];
    if (!name.startsWith('--') || value === undefined) {
      throw new Error(`Expected --name value pairs, received ${argv.join(' ')}`);
    }
    options[name.slice(2)] = value;
  }
  return options;
}

function requireOption(options, name) {
  const value = options[name];
  if (value === undefined || value === '') {
    throw new Error(`Missing --${name}`);
  }
  return value;
}

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  const options = parseArgs(rest);
  const cwd = process.cwd();

  if (command === 'prepare') {
    const branch = requireOption(options, 'branch');
    const version = requireOption(options, 'version');
    const currentVersion = readRolldownVersion(cwd);
    checkPrepareVersion({ branch, version, currentVersion });
    console.log(`Preparing v${version} on ${branch} (current: ${currentVersion}).`);
  } else if (command === 'publish') {
    // Prints step outputs; redirect stdout to $GITHUB_OUTPUT.
    const branch = requireOption(options, 'branch');
    const subject = requireOption(options, 'subject');
    const version = readRolldownVersion(cwd);
    const npmLatest =
      parseReleaseSubject(subject) === undefined ? undefined : await fetchNpmLatest('rolldown');
    const result = resolvePublish({ branch, subject, version, npmLatest });
    if (!result.release) {
      console.log('release=false');
      return;
    }
    console.log(
      [
        'release=true',
        `version=${result.version}`,
        `dist_tag=${result.distTag}`,
        `make_latest=${result.makeLatest}`,
      ].join('\n'),
    );
  } else {
    throw new Error(`Unknown command ${command ?? '(none)'}; expected prepare or publish`);
  }
}

main().catch((error) => {
  console.error(`::error::${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});

import { existsSync, readFileSync } from 'node:fs';
import { readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

/** The registry used by the release workflow. */
const DEFAULT_NPM_REGISTRY = 'https://registry.npmjs.org';

/**
 * npm installs resolve versions from the abbreviated packument, which is
 * cached separately from the full package document.
 */
const ABBREVIATED_PACKUMENT_ACCEPT = 'application/vnd.npm.install-v1+json';

/**
 * npm publish-time scanning can hold native binding tarballs for a long time
 * (about 90 minutes in https://github.com/rolldown/rolldown/issues/10721).
 * Fail the release rather than publish `rolldown` against missing optionals.
 */
const DEFAULT_TIMEOUT_SECONDS = 7200;
const DEFAULT_MIN_SECONDS = 60;
const DEFAULT_POLL_SECONDS = 15;

async function fetchNpmPackument(name, options) {
  const registry = options.registry.replace(/\/+$/, '');
  const response = await options.fetchImpl(`${registry}/${name.replace('/', '%2f')}`, {
    headers: { accept: ABBREVIATED_PACKUMENT_ACCEPT },
    signal: options.signal,
  });
  if (response.status === 404) {
    return null;
  }
  if (!response.ok) {
    throw new Error(`registry returned HTTP ${response.status}`);
  }
  return await response.json();
}

/**
 * Checks the same metadata document an npm install uses, then verifies that
 * the tarball referenced by that document can also be fetched.
 */
async function isNpmPackageAvailable(pkg, options) {
  const packument = await fetchNpmPackument(pkg.name, options);
  const tarball = packument?.versions?.[pkg.version]?.dist?.tarball;
  if (!tarball) {
    return false;
  }

  const tarballResponse = await options.fetchImpl(tarball, {
    method: 'HEAD',
    signal: options.signal,
  });
  return tarballResponse.ok;
}

function propagationTimeoutError(pending, timeoutSeconds) {
  const packageList = [...pending].map(([name, version]) => `${name}@${version}`).join(', ');
  return new Error(
    `Timed out after ${timeoutSeconds}s waiting for npm propagation: ${packageList}`,
  );
}

/**
 * Waits until every package version is installable before the release moves
 * on to a package that pins it as a dependency.
 */
async function waitForNpmPackages(packages, options) {
  if (packages.length === 0) {
    return;
  }

  const start = options.now();
  const deadline = start + options.timeoutSeconds * 1000;
  const pending = new Map(packages.map((pkg) => [pkg.name, pkg.version]));

  options.log(`Waiting for ${pending.size} npm package version(s) to become installable...`);

  while (pending.size > 0) {
    const remainingMilliseconds = deadline - options.now();
    if (remainingMilliseconds <= 0) {
      throw propagationTimeoutError(pending, options.timeoutSeconds);
    }

    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), remainingMilliseconds);
    try {
      const results = await Promise.all(
        [...pending].map(async ([name, version]) => {
          try {
            const available = await isNpmPackageAvailable(
              { name, version },
              { ...options, signal: controller.signal },
            );
            return { name, version, available, error: null };
          } catch (error) {
            return { name, version, available: false, error };
          }
        }),
      );

      if (controller.signal.aborted || options.now() >= deadline) {
        throw propagationTimeoutError(pending, options.timeoutSeconds);
      }

      for (const result of results) {
        if (result.available) {
          options.log(`  ${result.name}@${result.version}: available`);
          pending.delete(result.name);
        } else if (result.error) {
          options.log(
            `  ${result.name}@${result.version}: check failed, retrying (${String(result.error)})`,
          );
        }
      }
    } finally {
      clearTimeout(timeout);
    }

    if (pending.size === 0) {
      break;
    }
    const remainingAfterRound = deadline - options.now();
    if (remainingAfterRound <= 0) {
      throw propagationTimeoutError(pending, options.timeoutSeconds);
    }
    await options.sleep(Math.min(options.pollSeconds * 1000, remainingAfterRound));
  }

  if (options.minSeconds > 0) {
    const elapsedSeconds = Math.round((options.now() - start) / 1000);
    options.log(
      `All versions are installable after ${elapsedSeconds}s; settling for a further ${options.minSeconds}s.`,
    );
    await options.sleep(options.minSeconds * 1000);
  }
}

function parseNpmPackageSpec(spec) {
  const separator = spec.lastIndexOf('@');
  if (separator <= 0 || separator === spec.length - 1) {
    throw new Error(`Expected a package argument in the form name@version, received ${spec}`);
  }
  return { name: spec.slice(0, separator), version: spec.slice(separator + 1) };
}

function readPackageJsonNameVersion(packageJsonPath) {
  const pkg = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
  if (typeof pkg.name !== 'string' || typeof pkg.version !== 'string') {
    throw new Error(`Expected name and version in ${packageJsonPath}`);
  }
  return { name: pkg.name, version: pkg.version };
}

/**
 * Binding packages that napi actually uploaded. napi skips targets whose
 * `.node` / `.wasm` file is missing, but still lists every target in
 * `optionalDependencies`, so we must not wait on those missing packages.
 */
async function collectPublishedBindingPackages(npmDir) {
  const packages = [];
  if (!existsSync(npmDir)) {
    throw new Error(`Binding npm directory does not exist: ${npmDir}`);
  }

  const entries = await readdir(npmDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const dir = join(npmDir, entry.name);
    const packageJsonPath = join(dir, 'package.json');
    if (!existsSync(packageJsonPath)) {
      continue;
    }
    const files = await readdir(dir);
    const hasBinary = files.some((file) => file.endsWith('.node') || file.endsWith('.wasm'));
    if (!hasBinary) {
      continue;
    }
    packages.push(readPackageJsonNameVersion(packageJsonPath));
  }

  packages.sort((a, b) => a.name.localeCompare(b.name));
  return packages;
}

function readNonNegativeInteger(name, fallback) {
  const value = process.env[name];
  if (value === undefined || value.trim() === '') {
    return fallback;
  }
  if (!/^\d+$/.test(value)) {
    throw new Error(`Expected ${name} to be a non-negative integer, received ${value}`);
  }
  return Number(value);
}

/** Uses the release workflow's propagation settings. */
async function waitForNpmPackagesFromEnv(packages) {
  if (packages.length === 0) {
    throw new Error(
      'No npm packages to wait for. Pass name@version, --from-package-json, or --from-binding-dirs.',
    );
  }

  if (process.env.PUBLISH_SKIP_PROPAGATION_WAIT === 'true') {
    console.log('Skipping npm propagation wait.');
    return;
  }

  await waitForNpmPackages(packages, {
    registry: process.env.PUBLISH_REGISTRY ?? DEFAULT_NPM_REGISTRY,
    fetchImpl: fetch,
    minSeconds: readNonNegativeInteger('PUBLISH_PROPAGATION_MIN_SECONDS', DEFAULT_MIN_SECONDS),
    timeoutSeconds: readNonNegativeInteger(
      'PUBLISH_PROPAGATION_TIMEOUT_SECONDS',
      DEFAULT_TIMEOUT_SECONDS,
    ),
    pollSeconds: readNonNegativeInteger('PUBLISH_PROPAGATION_POLL_SECONDS', DEFAULT_POLL_SECONDS),
    sleep,
    now: Date.now,
    log: console.log,
  });
}

function parseArgs(argv) {
  const specs = [];
  const packageJsonFiles = [];
  const bindingDirs = [];

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--from-binding-dirs') {
      const dir = argv[++i];
      if (!dir) {
        throw new Error('Expected a directory after --from-binding-dirs');
      }
      bindingDirs.push(dir);
    } else if (arg === '--from-package-json') {
      const file = argv[++i];
      if (!file) {
        throw new Error('Expected a path after --from-package-json');
      }
      packageJsonFiles.push(file);
    } else if (arg.startsWith('-')) {
      throw new Error(`Unknown option: ${arg}`);
    } else {
      specs.push(arg);
    }
  }

  return { specs, packageJsonFiles, bindingDirs };
}

async function collectPackagesFromArgs(argv, cwd = process.cwd()) {
  const { specs, packageJsonFiles, bindingDirs } = parseArgs(argv);
  const packages = specs.map((spec) => parseNpmPackageSpec(spec));

  for (const file of packageJsonFiles) {
    packages.push(readPackageJsonNameVersion(resolve(cwd, file)));
  }
  for (const dir of bindingDirs) {
    packages.push(...(await collectPublishedBindingPackages(resolve(cwd, dir))));
  }

  return packages;
}

async function main() {
  const packages = await collectPackagesFromArgs(process.argv.slice(2));
  await waitForNpmPackagesFromEnv(packages);
}

const invokedPath = process.argv[1];
if (invokedPath && pathToFileURL(resolve(invokedPath)).href === import.meta.url) {
  main().catch((error) => {
    console.error(`::error::${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
}

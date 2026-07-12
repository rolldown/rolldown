import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, realpath, stat, symlink, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  CORPUS_ROOT,
  PRIMARY_ADMISSION_ORDER,
  assertLocalNode,
  projectDefinition,
  projectRoot,
} from './projects.mjs';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));

function runGit(arguments_, { cwd, allowFailure = false } = {}) {
  const result = spawnSync('git', arguments_, { cwd, encoding: 'utf8' });
  if (!allowFailure && result.status !== 0) {
    throw new Error(`git ${arguments_.join(' ')} failed:\n${result.stderr}`);
  }
  return result;
}

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = nodePath.join(directory, entry.name);
      if (entry.name === '.git' || entry.name === 'node_modules') return [];
      return entry.isDirectory() ? walk(path) : [path];
    }),
  );
  return nested.flat();
}

async function writeSparsePatterns(root, patterns) {
  const path = nodePath.join(root, '.git/info/sparse-checkout');
  await mkdir(nodePath.dirname(path), { recursive: true });
  await writeFile(path, `${patterns.join('\n')}\n`);
  runGit(['-C', root, 'config', 'core.sparseCheckout', 'true']);
  runGit(['-C', root, 'config', 'core.sparseCheckoutCone', 'false']);
}

async function checkoutPinnedProject(project) {
  const root = projectRoot(project.id);
  await mkdir(CORPUS_ROOT, { recursive: true });
  const gitDirectory = nodePath.join(root, '.git');
  let initialized = true;
  try {
    await stat(gitDirectory);
  } catch {
    initialized = false;
  }
  if (!initialized) {
    await mkdir(root, { recursive: true });
    runGit(['init', '-q', root]);
    runGit(['-C', root, 'remote', 'add', 'origin', project.repository]);
  }
  if (project.sparsePaths) await writeSparsePatterns(root, project.sparsePaths);

  const hasCommit = runGit(['-C', root, 'cat-file', '-e', `${project.commit}^{commit}`], {
    allowFailure: true,
  });
  if (hasCommit.status !== 0) {
    runGit(['-C', root, 'fetch', '--depth=1', '--filter=blob:none', 'origin', project.commit]);
  }
  runGit(['-C', root, 'checkout', '--quiet', '--detach', project.commit]);
  if (project.sparsePaths) runGit(['-C', root, 'read-tree', '-mu', 'HEAD']);

  const head = runGit(['-C', root, 'rev-parse', 'HEAD']).stdout.trim();
  if (head !== project.commit) throw new Error(`${project.id} HEAD differs from frozen pin`);
  const status = runGit(['-C', root, 'status', '--short']).stdout.trim();
  if (status) throw new Error(`${project.id} checkout is dirty:\n${status}`);
  return root;
}

async function inspectLicense(project, root) {
  if (project.license.path) {
    const content = await readFile(nodePath.join(root, project.license.path));
    const actual = sha256(content);
    if (actual !== project.license.sha256) {
      throw new Error(`${project.id} license hash mismatch: ${actual}`);
    }
    return { ...project.license, bytes: content.byteLength };
  }
  const declaration = JSON.parse(
    await readFile(nodePath.join(root, project.license.declarationPath), 'utf8'),
  );
  const value = declaration[project.license.declarationField];
  if (value !== project.license.declarationValue) {
    throw new Error(`${project.id} license declaration mismatch: ${value}`);
  }
  return { ...project.license };
}

async function collectSfcManifest(project, root) {
  const uniquePaths = new Set();
  const canonicalRoot =
    project.sfcRoots.length === 1 ? nodePath.resolve(root, project.sfcRoots[0]) : root;
  for (const relativeRoot of project.sfcRoots) {
    for (const path of await walk(nodePath.resolve(root, relativeRoot))) {
      if (path.endsWith('.vue')) uniquePaths.add(path);
    }
  }
  const entries = [];
  let bytes = 0;
  let canonical = '';
  for (const path of [...uniquePaths].sort((left, right) =>
    byteSort(nodePath.relative(root, left), nodePath.relative(root, right)),
  )) {
    const content = await readFile(path);
    const relativePath = nodePath.relative(root, path).split(nodePath.sep).join('/');
    const manifestPath = nodePath.relative(canonicalRoot, path).split(nodePath.sep).join('/');
    const contentSha256 = sha256(content);
    bytes += content.byteLength;
    canonical += `${manifestPath}\0${content.byteLength}\0${contentSha256}\n`;
    entries.push({ path: relativePath, bytes: content.byteLength, sha256: contentSha256 });
  }
  const manifestSha256 = sha256(canonical);
  if (entries.length !== project.expectedPhysicalSfcCount) {
    throw new Error(
      `${project.id} physical SFC count mismatch: ${entries.length} != ${project.expectedPhysicalSfcCount}`,
    );
  }
  if (
    project.expectedPhysicalSfcBytes !== undefined &&
    bytes !== project.expectedPhysicalSfcBytes
  ) {
    throw new Error(`${project.id} physical SFC byte mismatch: ${bytes}`);
  }
  if (
    project.expectedPhysicalSfcManifestSha256 !== undefined &&
    manifestSha256 !== project.expectedPhysicalSfcManifestSha256
  ) {
    throw new Error(`${project.id} physical SFC manifest mismatch: ${manifestSha256}`);
  }
  return { count: entries.length, bytes, manifestSha256, entries };
}

async function countSfcUnderRoots(root, roots) {
  let count = 0;
  for (const relativeRoot of roots) {
    count += (await walk(nodePath.join(root, relativeRoot))).filter((path) =>
      path.endsWith('.vue'),
    ).length;
  }
  return count;
}

async function ensureWorkspaceLink(root, packageName, target) {
  const segments = packageName.split('/');
  const link = nodePath.join(root, 'node_modules', ...segments);
  await mkdir(nodePath.dirname(link), { recursive: true });
  try {
    const actual = await realpath(link);
    const expected = await realpath(target);
    if (actual !== expected) {
      throw new Error(
        `workspace support link ${packageName} points to ${actual}, expected ${expected}`,
      );
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    await symlink(nodePath.relative(nodePath.dirname(link), target), link, 'dir');
  }
  return { packageName, target: nodePath.relative(root, target).split(nodePath.sep).join('/') };
}

function parseModulesMetadata(source) {
  if (source.trimStart().startsWith('{')) {
    const value = JSON.parse(source);
    return {
      packageManager: value.packageManager,
      layoutVersion: value.layoutVersion,
      nodeLinker: value.nodeLinker,
      virtualStoreDir: value.virtualStoreDir,
      included: value.included,
    };
  }
  const scalar = (name) => {
    const match = source.match(new RegExp(`^${name}:\\s*['"]?([^'"\\n]+)['"]?$`, 'm'));
    return match?.[1];
  };
  const boolean = (name) => {
    const match = source.match(new RegExp(`^  ${name}:\\s*(true|false)$`, 'm'));
    return match ? match[1] === 'true' : undefined;
  };
  return {
    packageManager: scalar('packageManager'),
    layoutVersion: Number(scalar('layoutVersion')),
    nodeLinker: scalar('nodeLinker'),
    virtualStoreDir: scalar('virtualStoreDir'),
    included: {
      dependencies: boolean('dependencies'),
      devDependencies: boolean('devDependencies'),
      optionalDependencies: boolean('optionalDependencies'),
    },
  };
}

async function inspectPnpmInstall(root, project) {
  const expectation = project.dependencyPreparation;
  const packageJsonPath = nodePath.join(root, 'package.json');
  const packageJsonBytes = await readFile(packageJsonPath);
  const packageJson = JSON.parse(packageJsonBytes);
  if (packageJson.packageManager !== expectation.packageManager) {
    throw new Error(
      `${project.id} packageManager mismatch: ${packageJson.packageManager} != ${expectation.packageManager}`,
    );
  }
  const rootLockfile = await readFile(nodePath.join(root, 'pnpm-lock.yaml'));
  const rootLockfileSha256 = sha256(rootLockfile);
  if (rootLockfileSha256 !== expectation.rootLockfileSha256) {
    throw new Error(`${project.id} root pnpm lockfile hash mismatch: ${rootLockfileSha256}`);
  }
  const modulesBytes = await readFile(nodePath.join(root, 'node_modules/.modules.yaml'));
  const installLockfile = await readFile(nodePath.join(root, 'node_modules/.pnpm/lock.yaml'));
  const metadata = parseModulesMetadata(modulesBytes.toString('utf8'));
  if (metadata.packageManager !== expectation.packageManager) {
    throw new Error(
      `${project.id} node_modules was installed by ${metadata.packageManager}, expected ${expectation.packageManager}`,
    );
  }
  if (
    metadata.layoutVersion !== 5 ||
    metadata.nodeLinker !== 'isolated' ||
    metadata.virtualStoreDir !== '.pnpm' ||
    !metadata.included?.dependencies ||
    !metadata.included?.devDependencies ||
    !metadata.included?.optionalDependencies
  ) {
    throw new Error(`${project.id} node_modules install metadata differs from the frozen layout`);
  }
  const installLockfileSha256 = sha256(installLockfile);
  if (installLockfileSha256 !== expectation.installLockfileSha256) {
    throw new Error(
      `${project.id} installed pnpm lockfile hash mismatch: ${installLockfileSha256}`,
    );
  }
  const criticalPackages = [];
  for (const expected of expectation.criticalPackages) {
    const content = await readFile(nodePath.join(root, expected.path));
    const packageJson_ = JSON.parse(content);
    const actual = { path: expected.path, version: packageJson_.version, sha256: sha256(content) };
    if (actual.version !== expected.version || actual.sha256 !== expected.sha256) {
      throw new Error(
        `${project.id} critical package drift at ${expected.path}: ${actual.version}/${actual.sha256}`,
      );
    }
    criticalPackages.push(actual);
  }
  return {
    packageManager: expectation.packageManager,
    rootPackageJsonSha256: sha256(packageJsonBytes),
    rootLockfileSha256,
    installLockfileSha256,
    modulesMetadata: metadata,
    modulesMetadataSha256: sha256(JSON.stringify(metadata)),
    criticalPackages,
  };
}

function runPinnedPnpm(root, project, arguments_) {
  const corepack = nodePath.join(nodePath.dirname(process.execPath), 'corepack');
  const result = spawnSync(
    corepack,
    [project.dependencyPreparation.packageManager, ...arguments_],
    {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 32 * 1024 * 1024,
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `corepack ${project.dependencyPreparation.packageManager} ${arguments_.join(' ')} failed:\n${result.stderr}`,
    );
  }
  return result;
}

async function prepareVbenWorkspaceLinks(root) {
  const packageFiles = (await walk(root)).filter(
    (path) =>
      nodePath.basename(path) === 'package.json' &&
      (path.includes(`${nodePath.sep}packages${nodePath.sep}`) ||
        path === nodePath.join(root, 'internal/tsconfig/package.json')),
  );
  const links = [];
  for (const packagePath of packageFiles.sort()) {
    const packageJson = JSON.parse(await readFile(packagePath, 'utf8'));
    if (typeof packageJson.name === 'string') {
      links.push(await ensureWorkspaceLink(root, packageJson.name, nodePath.dirname(packagePath)));
    }
  }
  return links;
}

async function prepareVbenDependencies(root, project) {
  const packageJson = JSON.parse(await readFile(nodePath.join(root, 'package.json'), 'utf8'));
  const sentinels = [
    'apps/web-antd/node_modules/vue/package.json',
    'packages/@core/ui-kit/shadcn-ui/node_modules/reka-ui/package.json',
    'packages/@core/ui-kit/form-ui/node_modules/unplugin-vue/package.json',
  ];
  let installRequired = false;
  for (const sentinel of sentinels) {
    try {
      await readFile(nodePath.join(root, sentinel));
    } catch {
      installRequired = true;
    }
  }
  const command = [
    'install',
    '--filter',
    '@vben/web-antd...',
    '--ignore-scripts',
    '--frozen-lockfile',
  ];
  if (installRequired) {
    runPinnedPnpm(root, project, command);
  }
  const version = runPinnedPnpm(root, project, ['--version']).stdout.trim();
  if (`pnpm@${version}` !== project.dependencyPreparation.packageManager) {
    throw new Error(`${project.id} Corepack resolved unexpected pnpm ${version}`);
  }
  const dependencyLinks = [];
  for (const [packageName, target] of [
    ['vue', nodePath.join(root, 'apps/web-antd/node_modules/vue')],
  ]) {
    dependencyLinks.push(await ensureWorkspaceLink(root, packageName, target));
  }
  return {
    packageManager: packageJson.packageManager,
    invokedThrough: nodePath.join(nodePath.dirname(process.execPath), 'corepack'),
    invokedPnpmVersion: version,
    command: `corepack ${project.dependencyPreparation.packageManager} ${command.join(' ')}`,
    scriptsEnabled: false,
    installPerformed: installRequired,
    sentinels,
    rootDependencyLinks: dependencyLinks,
    install: await inspectPnpmInstall(root, project),
  };
}

async function prepareDirectusDependencies(root, project) {
  const packageJson = JSON.parse(await readFile(nodePath.join(root, 'package.json'), 'utf8'));
  const sentinels = [
    'app/node_modules/vue/package.json',
    'app/node_modules/@directus/tsconfig/package.json',
    'app/node_modules/@vitejs/plugin-vue/package.json',
  ];
  let installRequired = false;
  for (const sentinel of sentinels) {
    try {
      await readFile(nodePath.join(root, sentinel));
    } catch {
      installRequired = true;
    }
  }
  const command = [
    'install',
    '--filter',
    '@directus/app...',
    '--ignore-scripts',
    '--frozen-lockfile',
  ];
  if (installRequired) {
    runPinnedPnpm(root, project, command);
  }
  const version = runPinnedPnpm(root, project, ['--version']).stdout.trim();
  if (`pnpm@${version}` !== project.dependencyPreparation.packageManager) {
    throw new Error(`${project.id} Corepack resolved unexpected pnpm ${version}`);
  }
  const rootDependencyLinks = [
    await ensureWorkspaceLink(root, 'vue', nodePath.join(root, 'app/node_modules/vue')),
  ];
  const vuePackage = JSON.parse(
    await readFile(nodePath.join(root, 'app/node_modules/vue/package.json'), 'utf8'),
  );
  return {
    packageManager: packageJson.packageManager,
    packageNodeEngine: packageJson.engines?.node,
    invokedNodeVersion: process.version,
    invokedThrough: nodePath.join(nodePath.dirname(process.execPath), 'corepack'),
    invokedPnpmVersion: version,
    command: `corepack ${project.dependencyPreparation.packageManager} ${command.join(' ')}`,
    scriptsEnabled: false,
    installPerformed: installRequired,
    sentinels,
    rootDependencyLinks,
    projectVueVersion: vuePackage.version,
    install: await inspectPnpmInstall(root, project),
  };
}

export async function ensurePreparedProject(projectId) {
  assertLocalNode();
  const project = projectDefinition(projectId);
  const root = await checkoutPinnedProject(project);
  const [license, sfc] = await Promise.all([
    inspectLicense(project, root),
    collectSfcManifest(project, root),
  ]);
  for (const entry of project.entries ?? []) await readFile(nodePath.join(root, entry));
  const dependencyPreparation =
    project.id === 'vben'
      ? await prepareVbenDependencies(root, project)
      : project.id === 'directus-amendment-candidate'
        ? await prepareDirectusDependencies(root, project)
        : undefined;
  const workspaceSupportLinks =
    project.id === 'vben' ? await prepareVbenWorkspaceLinks(root) : undefined;
  const reachableEnvelopeSfcCount = project.reachableEnvelopeRoots
    ? await countSfcUnderRoots(root, project.reachableEnvelopeRoots)
    : undefined;
  if (
    project.expectedReachableEnvelopeSfcCount !== undefined &&
    reachableEnvelopeSfcCount !== project.expectedReachableEnvelopeSfcCount
  ) {
    throw new Error(`${project.id} reachable-envelope SFC mismatch: ${reachableEnvelopeSfcCount}`);
  }
  return {
    projectId,
    band: project.band,
    repository: project.repository,
    commit: project.commit,
    root,
    license,
    entries: project.entries,
    entryGenerator: project.entryGenerator,
    physicalSfc: {
      count: sfc.count,
      bytes: sfc.bytes,
      manifestSha256: sfc.manifestSha256,
      paths: sfc.entries.map((entry) => entry.path),
    },
    reachableEnvelope: project.reachableEnvelopeRoots
      ? {
          roots: project.reachableEnvelopeRoots,
          sfcCount: reachableEnvelopeSfcCount,
          minimumAdmissionSfcCount: project.minimumReachedSfcCount,
          structurallyCanMeetMinimum: reachableEnvelopeSfcCount >= project.minimumReachedSfcCount,
        }
      : undefined,
    workspaceSupportLinks,
    dependencyPreparation,
  };
}

function parseSelection(arguments_) {
  if (arguments_.includes('--all')) return [...PRIMARY_ADMISSION_ORDER, 'vben'];
  if (arguments_.includes('--fallback')) return ['vben'];
  const selected = [];
  for (let index = 0; index < arguments_.length; index++) {
    if (arguments_[index] === '--project') selected.push(arguments_[++index]);
  }
  return selected.length === 0 ? PRIMARY_ADMISSION_ORDER : selected;
}

if (process.argv[1] === import.meta.filename) {
  assertLocalNode();
  const selected = parseSelection(process.argv.slice(2));
  for (const projectId of selected) projectDefinition(projectId);
  const projects = [];
  for (const projectId of selected) projects.push(await ensurePreparedProject(projectId));
  console.log(
    JSON.stringify(
      {
        schema: 1,
        node: process.version,
        selected,
        projects,
      },
      null,
      2,
    ),
  );
}

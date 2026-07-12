import { createHash } from 'node:crypto';
import { lstat, readFile, readdir, stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import nodePath from 'node:path';
import { REPOSITORY_ROOT, REQUIRED_NODE_VERSION } from './projects.mjs';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const TREE_ALGORITHM =
  'SHA-256 over every UTF-8-sorted package-payload regular file excluding package-local node_modules: relative path + NUL + bytes + NUL + content SHA-256 + LF';
const ADAPTER_ROOT = nodePath.resolve(import.meta.dirname, '../../parallel-vue-plugin');
const ADAPTER_IMPLEMENTATION_PATH = nodePath.join(ADAPTER_ROOT, 'impl.js');
const ADAPTER_REQUIRE = createRequire(ADAPTER_IMPLEMENTATION_PATH);

export const ADAPTER_BASE_PROVENANCE_EXPECTATION = Object.freeze({
  kind: 'independent Vue adapter toolchain provenance',
  node: Object.freeze({
    version: 'v24.18.0',
    bytes: 120_965_360,
    sha256: 'ee6fb0e015284d83a91e8ec5213f43a157f8a392b58555301682892ba928c04a',
  }),
  installation: Object.freeze({
    files: Object.freeze([
      Object.freeze({
        path: 'examples/par-plugin/package.json',
        bytes: 1033,
        sha256: '164755bc240c737a95f9704add2eb64ae4c246195a0d0f2e8f28da68d77c7262',
      }),
      Object.freeze({
        path: 'node_modules/.pnpm/lock.yaml',
        bytes: 553_161,
        sha256: '2cc3710e028dd31f108d5ac993aca4205bddf126099b6d474a2321f8c2ba170a',
      }),
      Object.freeze({
        path: 'package.json',
        bytes: 1277,
        sha256: 'a56f43a5c1df0cf52223d60e288a265354889dc01fa7126b9a80a56893b21fc7',
      }),
      Object.freeze({
        path: 'pnpm-lock.yaml',
        bytes: 553_161,
        sha256: '2cc3710e028dd31f108d5ac993aca4205bddf126099b6d474a2321f8c2ba170a',
      }),
      Object.freeze({
        path: 'pnpm-workspace.yaml',
        bytes: 3390,
        sha256: 'a003e5867870bfdb4bcf76331db32d01ecc1a0153d97ee1703dd401b48cc1540',
      }),
    ]),
    manifestSha256: 'ba88330c03e9323b6abfea68d484312f37cbabbedd117441ed3b67c8e2e2c522',
  }),
  modulesMetadata: Object.freeze({
    packageManager: 'pnpm@11.9.0',
    layoutVersion: 5,
    nodeLinker: 'isolated',
    virtualStoreDir: '.pnpm',
    included: Object.freeze({
      dependencies: true,
      devDependencies: true,
      optionalDependencies: true,
    }),
  }),
  unpluginVue: Object.freeze({
    version: '7.2.0',
    packageJsonBytes: 2436,
    packageJsonSha256: 'e5e394d8ace1faccb05048e3c9da899aab57ec39f92dc5ec6ab46ea684690815',
    entrypoint: Object.freeze({
      path: 'dist/rolldown.mjs',
      bytes: 400,
      sha256: 'cf2382afdc0bc12df208f49b879cfbc9c350beda4b76c85b95ca8a7e0fc69374',
    }),
    tree: Object.freeze({
      algorithm: TREE_ALGORITHM,
      files: 23,
      bytes: 56_484,
      sha256: '92cc413985d0b7731e9e3de77d5a30057671065199ca13662cc5bb2578e37a4c',
    }),
  }),
  source: Object.freeze({
    files: Object.freeze([
      Object.freeze({
        path: 'impl.js',
        bytes: 5791,
        sha256: '7c30524ba7e9eed355ef55d381133271be9bdc6fe3e8f5b525b06ef7f80808b5',
      }),
      Object.freeze({
        path: 'index.js',
        bytes: 186,
        sha256: 'ef39534320c3b94c4d9198172e685ef2a71364b583287f4d89616c2877ca04ec',
      }),
      Object.freeze({
        path: 'metrics.js',
        bytes: 3745,
        sha256: '6c35e162306082cb1bdf89cdf82e845e99223f111577a05fd411d4cd7d9be2b6',
      }),
    ]),
    manifestSha256: '44e5c6521cd61df04982f7b1eb9315ace528f561a196164345bdd6e1d694dff2',
  }),
});

const COMPILER_PROVENANCE_EXPECTATION = Object.freeze({
  resolutionSource: 'adapter-explicit-option',
  request: 'vue/compiler-sfc',
  nodeWrapperPackage: Object.freeze({
    version: '3.5.39',
    packageJsonBytes: 2769,
    packageJsonSha256: 'a47a52a49abb476476237bfb7a6337fee5976d516bb5df1cf1354fd3b5eff288',
    entrypoint: Object.freeze({
      path: 'compiler-sfc/index.js',
      bytes: 75,
      sha256: 'a2226c3eedb827ce90fabd18413c8b6d4008a99269f403f86f160b733c546970',
    }),
    tree: Object.freeze({
      algorithm: TREE_ALGORITHM,
      files: 37,
      bytes: 2_503_852,
      sha256: 'e1ba4d4381c254c8b47390d12767d977724277d3640de21d024fb0e6b3665ab4',
    }),
  }),
  compilerPackage: Object.freeze({
    version: '3.5.39',
    packageJsonBytes: 1710,
    packageJsonSha256: '3ebcadb08b31e9207a7c5a2073e4ae33daa94bba601fc1c88634b63c1dba0b73',
    entrypoint: Object.freeze({
      path: 'dist/compiler-sfc.cjs.js',
      bytes: 886_681,
      sha256: '36048750a63359f1b062627946a2ca59d91f729af225f9da09fe84ac4917526b',
    }),
    tree: Object.freeze({
      algorithm: TREE_ALGORITHM,
      files: 6,
      bytes: 2_622_623,
      sha256: '47016522990cc6394c590d9c3a0f837da39ff0a7ee22d2d7b4b1f25b6345d640',
    }),
  }),
});

async function findPackage(entryPath, expectedName) {
  let directory = nodePath.dirname(entryPath);
  while (directory !== nodePath.dirname(directory)) {
    const packageJsonPath = nodePath.join(directory, 'package.json');
    try {
      const packageJsonBytes = await readFile(packageJsonPath);
      const packageJson = JSON.parse(packageJsonBytes);
      if (packageJson.name === expectedName) {
        return { root: directory, packageJsonPath, packageJsonBytes, packageJson };
      }
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    directory = nodePath.dirname(directory);
  }
  throw new Error(`could not locate package.json for ${expectedName}`);
}

async function collectPackageTree(root) {
  const entries = [];
  const visit = async (directory) => {
    const children = await readdir(directory, { withFileTypes: true });
    children.sort((left, right) => byteSort(left.name, right.name));
    for (const child of children) {
      if (directory === root && child.name === 'node_modules') continue;
      const path = nodePath.join(directory, child.name);
      const pathStat = await lstat(path);
      if (pathStat.isDirectory()) {
        await visit(path);
        continue;
      }
      if (!pathStat.isFile()) {
        throw new Error(`package tree contains a non-regular entry: ${path}`);
      }
      const content = await readFile(path);
      entries.push({
        path: nodePath.relative(root, path).split(nodePath.sep).join('/'),
        bytes: content.byteLength,
        sha256: sha256(content),
      });
    }
  };
  await visit(root);
  entries.sort((left, right) => byteSort(left.path, right.path));
  const canonical = entries
    .map((entry) => `${entry.path}\0${entry.bytes}\0${entry.sha256}\n`)
    .join('');
  return {
    algorithm: TREE_ALGORITHM,
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    sha256: sha256(canonical),
  };
}

async function inspectPackage(entryPath, expectedName) {
  const package_ = await findPackage(entryPath, expectedName);
  const entrypointBytes = await readFile(entryPath);
  const relativeEntrypoint = nodePath
    .relative(package_.root, entryPath)
    .split(nodePath.sep)
    .join('/');
  if (relativeEntrypoint.startsWith('../') || nodePath.isAbsolute(relativeEntrypoint)) {
    throw new Error(`${expectedName} entrypoint is outside its package root`);
  }
  const actual = {
    version: package_.packageJson.version,
    packageJsonBytes: package_.packageJsonBytes.byteLength,
    packageJsonSha256: sha256(package_.packageJsonBytes),
    entrypoint: {
      path: relativeEntrypoint,
      bytes: entrypointBytes.byteLength,
      sha256: sha256(entrypointBytes),
    },
    tree: await collectPackageTree(package_.root),
  };
  return actual;
}

async function inspectFileSet(definitions) {
  const files = [];
  for (const definition of definitions) {
    const bytes = await readFile(definition.absolutePath);
    files.push({ path: definition.path, bytes: bytes.byteLength, sha256: sha256(bytes) });
  }
  files.sort((left, right) => byteSort(left.path, right.path));
  return {
    files,
    manifestSha256: sha256(
      files.map((file) => `${file.path}\0${file.bytes}\0${file.sha256}\n`).join(''),
    ),
  };
}

export async function captureAdapterBaseProvenance({ verify = true } = {}) {
  if (process.version !== REQUIRED_NODE_VERSION) {
    throw new Error(`adapter provenance requires ${REQUIRED_NODE_VERSION}, got ${process.version}`);
  }
  const unpluginEntryPath = ADAPTER_REQUIRE.resolve('unplugin-vue/rolldown');
  const source = await inspectFileSet(
    ['impl.js', 'index.js', 'metrics.js'].map((name) => ({
      path: name,
      absolutePath: nodePath.join(ADAPTER_ROOT, name),
    })),
  );
  const installation = await inspectFileSet([
    { path: 'package.json', absolutePath: nodePath.join(REPOSITORY_ROOT, 'package.json') },
    {
      path: 'examples/par-plugin/package.json',
      absolutePath: nodePath.join(REPOSITORY_ROOT, 'examples/par-plugin/package.json'),
    },
    { path: 'pnpm-lock.yaml', absolutePath: nodePath.join(REPOSITORY_ROOT, 'pnpm-lock.yaml') },
    {
      path: 'pnpm-workspace.yaml',
      absolutePath: nodePath.join(REPOSITORY_ROOT, 'pnpm-workspace.yaml'),
    },
    {
      path: 'node_modules/.pnpm/lock.yaml',
      absolutePath: nodePath.join(REPOSITORY_ROOT, 'node_modules/.pnpm/lock.yaml'),
    },
  ]);
  const workspaceLock = installation.files.find(({ path }) => path === 'pnpm-lock.yaml');
  const installedLock = installation.files.find(
    ({ path }) => path === 'node_modules/.pnpm/lock.yaml',
  );
  if (
    workspaceLock.bytes !== installedLock.bytes ||
    workspaceLock.sha256 !== installedLock.sha256
  ) {
    throw new Error('installed pnpm lock differs from the committed workspace lock');
  }
  const nodeBinaryBytes = await readFile(process.execPath);
  const nodeBinaryStat = await stat(process.execPath);
  const modulesMetadata = JSON.parse(
    await readFile(nodePath.join(REPOSITORY_ROOT, 'node_modules/.modules.yaml')),
  );
  const actual = {
    kind: 'independent Vue adapter toolchain provenance',
    node: {
      version: process.version,
      bytes: nodeBinaryStat.size,
      sha256: sha256(nodeBinaryBytes),
    },
    installation,
    modulesMetadata: {
      packageManager: modulesMetadata.packageManager,
      layoutVersion: modulesMetadata.layoutVersion,
      nodeLinker: modulesMetadata.nodeLinker,
      virtualStoreDir: modulesMetadata.virtualStoreDir,
      included: modulesMetadata.included,
    },
    unpluginVue: await inspectPackage(unpluginEntryPath, 'unplugin-vue'),
    source,
  };
  return verify
    ? assertExactAdapterProvenance(
        actual,
        ADAPTER_BASE_PROVENANCE_EXPECTATION,
        'adapter base provenance',
      )
    : actual;
}

export async function captureProjectAdapterProvenance(
  projectId,
  _projectRoot,
  baseProvenance,
  { verify = true } = {},
) {
  const base = baseProvenance ?? (await captureAdapterBaseProvenance({ verify }));
  const wrapperEntrypointPath = ADAPTER_REQUIRE.resolve('vue/compiler-sfc');
  const compilerEntrypointPath = ADAPTER_REQUIRE.resolve('@vue/compiler-sfc');
  const actual = {
    ...base,
    projectId,
    compilerSfc: {
      resolutionSource: 'adapter-explicit-option',
      request: 'vue/compiler-sfc',
      nodeWrapperPackage: await inspectPackage(wrapperEntrypointPath, 'vue'),
      compilerPackage: await inspectPackage(compilerEntrypointPath, '@vue/compiler-sfc'),
    },
  };
  if (verify) {
    assertExactAdapterProvenance(
      actual.compilerSfc,
      COMPILER_PROVENANCE_EXPECTATION,
      `${projectId} compiler provenance`,
    );
  }
  return actual;
}

export function assertExactAdapterProvenance(actual, expected, label = 'adapter provenance') {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${label} drift`);
  }
  return actual;
}

export function assertFrozenProjectAdapterProvenance(actual, projectId) {
  if (!actual || actual.projectId !== projectId) {
    throw new Error(`${projectId} adapter provenance identity drift`);
  }
  const { compilerSfc, projectId: ignoredProjectId, ...base } = actual;
  void ignoredProjectId;
  assertExactAdapterProvenance(
    base,
    ADAPTER_BASE_PROVENANCE_EXPECTATION,
    'adapter base provenance',
  );
  assertExactAdapterProvenance(
    compilerSfc,
    COMPILER_PROVENANCE_EXPECTATION,
    `${projectId} compiler provenance`,
  );
  return actual;
}

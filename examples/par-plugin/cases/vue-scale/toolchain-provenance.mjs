import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { lstat, readFile, readdir, realpath } from 'node:fs/promises';
import nodePath from 'node:path';

const require = createRequire(import.meta.url);
const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

export const EXPECTED_VUE_TOOLCHAIN = {
  unpluginVue: {
    package: 'unplugin-vue',
    version: '7.2.0',
    files: 24,
    bytes: 59643,
    aggregateSha256: '2c7708305661427564c58ecf17d0189e072b670e98845d7d05220e39548a40e1',
    manifestSha256: 'e5e394d8ace1faccb05048e3c9da899aab57ec39f92dc5ec6ab46ea684690815',
    entrypoint: 'dist/rolldown.mjs',
    entrypointBytes: 400,
    entrypointSha256: 'cf2382afdc0bc12df208f49b879cfbc9c350beda4b76c85b95ca8a7e0fc69374',
  },
  compilerSfc: {
    package: '@vue/compiler-sfc',
    version: '3.5.39',
    files: 7,
    bytes: 2625197,
    aggregateSha256: '78778bd14ac76b778a7f3d953a6f2adb903a54a9404631a72e7ab56b594470d7',
    manifestSha256: '3ebcadb08b31e9207a7c5a2073e4ae33daa94bba601fc1c88634b63c1dba0b73',
    entrypoint: 'dist/compiler-sfc.cjs.js',
    entrypointBytes: 886681,
    entrypointSha256: '36048750a63359f1b062627946a2ca59d91f729af225f9da09fe84ac4917526b',
  },
};

export async function captureVueToolchainProvenance() {
  const captured = {
    unpluginVue: await capturePackage('unplugin-vue/package.json', 'unplugin-vue/rolldown'),
    compilerSfc: await capturePackage('@vue/compiler-sfc/package.json', '@vue/compiler-sfc'),
  };
  if (JSON.stringify(captured) !== JSON.stringify(EXPECTED_VUE_TOOLCHAIN)) {
    throw new Error(
      `resolved Vue transform toolchain differs from the frozen artifacts: ${JSON.stringify(captured)}`,
    );
  }
  return captured;
}

async function capturePackage(manifestSpecifier, entrypointSpecifier) {
  const manifestPath = await realpath(require.resolve(manifestSpecifier));
  const packageRoot = nodePath.dirname(manifestPath);
  const manifestContent = await readFile(manifestPath);
  const manifest = JSON.parse(manifestContent);
  const entries = [];
  await walk(packageRoot, packageRoot, entries);
  entries.sort((left, right) => compareUtf8(left.path, right.path));
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.path);
    aggregate.update('\0');
    aggregate.update(entry.kind);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  const entrypointPath = await realpath(require.resolve(entrypointSpecifier));
  const entrypointContent = await readFile(entrypointPath);
  return {
    package: manifest.name,
    version: manifest.version,
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    aggregateSha256: aggregate.digest('hex'),
    manifestSha256: sha256(manifestContent),
    entrypoint: nodePath.relative(packageRoot, entrypointPath).split(nodePath.sep).join('/'),
    entrypointBytes: entrypointContent.byteLength,
    entrypointSha256: sha256(entrypointContent),
  };
}

async function walk(root, directory, entries) {
  const directoryEntries = await readdir(directory, { withFileTypes: true });
  directoryEntries.sort((left, right) => compareUtf8(left.name, right.name));
  for (const entry of directoryEntries) {
    const path = nodePath.join(directory, entry.name);
    if (entry.isDirectory()) {
      await walk(root, path, entries);
      continue;
    }
    const fileStat = await lstat(path);
    if (!fileStat.isFile()) {
      throw new Error(`Vue toolchain package contains unsupported entry: ${path}`);
    }
    const content = await readFile(path);
    entries.push({
      path: nodePath.relative(root, path).split(nodePath.sep).join('/'),
      kind: 'file',
      bytes: content.byteLength,
      sha256: sha256(content),
    });
  }
}

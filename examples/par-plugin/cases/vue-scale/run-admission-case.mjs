import { createHash } from 'node:crypto';
import { realpath } from 'node:fs/promises';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { vueTransformPlugin } from '../../parallel-vue-plugin/impl.js';
import { listQuasarPreExclusionEntries, summarizeAdmissionEntries } from './admission-corpus.mjs';
import { readCorpusManifest } from './corpus.mjs';
import { REQUIRED_NODE_VERSION } from './provenance.mjs';

if (process.version !== REQUIRED_NODE_VERSION) {
  throw new Error(`Vue admission case requires Node.js ${REQUIRED_NODE_VERSION}`);
}
const options = JSON.parse(process.argv[2] ?? 'null');
if (!options || !['quasar-pre-exclusion', 'final-pool'].includes(options.phase)) {
  throw new Error('expected a supported Vue admission phase');
}
const corpusDirectory = await realpath(options.corpusDirectory);
const manifest = await readCorpusManifest(
  nodePath.join(import.meta.dirname, 'corpus-manifest.json'),
);
const entries =
  options.phase === 'quasar-pre-exclusion'
    ? await listQuasarPreExclusionEntries(corpusDirectory)
    : manifest.entries;
const entryId = '\0vue-scale-admission-entry';
const exportHelperId = '\0/plugin-vue/export-helper';
const entryCode = `${entries
  .map(
    (entry, index) =>
      `export { default as component_${String(index).padStart(4, '0')} } from ${JSON.stringify(nodePath.join(corpusDirectory, entry.sourceKey))};`,
  )
  .join('\n')}\n`;
const virtualModules = {
  name: 'vue-scale-admission-virtual-modules',
  resolveId(id) {
    if (id === entryId || id === exportHelperId) return id;
  },
  load(id) {
    if (id === entryId) return entryCode;
    if (id === exportHelperId) {
      return `export default (sfc, props) => {
  const target = sfc.__vccOpts || sfc;
  for (const [key, value] of props) target[key] = value;
  return target;
}`;
    }
  },
};

let build;
let failures = [];
let output;
try {
  build = await rolldown({
    cwd: corpusDirectory,
    input: entryId,
    logLevel: 'silent',
    moduleTypes: { vue: 'js' },
    resolve: { symlinks: false },
    treeshake: false,
    external: (_source, importer) => Boolean(importer && importer !== entryId),
    plugins: [virtualModules, vueTransformPlugin({ root: corpusDirectory })],
  });
  const generated = await build.generate({ format: 'esm', sourcemap: true });
  output = summarizeOutput(generated.output, corpusDirectory);
} catch (error) {
  if (!Array.isArray(error?.errors)) throw error;
  failures = error.errors.map((failure) => classifyFailure(failure, corpusDirectory));
  failures.sort((left, right) =>
    Buffer.compare(Buffer.from(left.sourceKey), Buffer.from(right.sourceKey)),
  );
} finally {
  await build?.close();
}

console.log(
  JSON.stringify({
    schema: 1,
    measurementClass: 'untimed compile admission; not performance evidence',
    phase: options.phase,
    selection: summarizeAdmissionEntries(entries),
    admitted: failures.length === 0,
    errorCount: failures.length,
    failures,
    output,
  }),
);

function classifyFailure(failure, root) {
  if (typeof failure?.id !== 'string') {
    throw new Error(`Vue admission failure omitted source id: ${failure?.message}`);
  }
  const nested =
    Array.isArray(failure.errors) && failure.errors.length === 1 ? failure.errors[0] : failure;
  const message = stripAnsi(String(nested?.message ?? failure.message ?? nested));
  const sourceKey = nodePath.relative(root, failure.id).split(nodePath.sep).join('/');
  if (sourceKey.startsWith('../') || nodePath.posix.isAbsolute(sourceKey)) {
    throw new Error(`Vue admission failure escaped the corpus: ${failure.id}`);
  }
  return {
    sourceKey,
    code: nested?.code ?? nested?.kind ?? failure.code ?? 'UNKNOWN',
    signature: message.includes('Tsconfig not found')
      ? 'TSCONFIG_ERROR: missing dependency of nearest tsconfig'
      : message.split('\n')[0],
    messageSha256: sha256(message),
  };
}

function summarizeOutput(values, root) {
  const code = createHash('sha256');
  const map = createHash('sha256');
  let chunks = 0;
  let assets = 0;
  let exports = 0;
  for (const value of [...values].sort((left, right) =>
    left.fileName.localeCompare(right.fileName),
  )) {
    if (value.type === 'asset') {
      assets++;
      code.update(
        `asset\0${value.fileName}\0${String(value.source).replaceAll(root, '<vue-scale-corpus>')}\0`,
      );
      continue;
    }
    chunks++;
    exports += value.exports.length;
    const sourceMap = typeof value.map === 'string' ? value.map : JSON.stringify(value.map);
    code.update(`${value.fileName}\0${value.code.replaceAll(root, '<vue-scale-corpus>')}\0`);
    map.update(`${value.fileName}\0${sourceMap.replaceAll(root, '<vue-scale-corpus>')}\0`);
  }
  return {
    chunks,
    assets,
    exports,
    normalizedCodeSha256: code.digest('hex'),
    normalizedMapSha256: map.digest('hex'),
  };
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function stripAnsi(value) {
  return value.replaceAll(new RegExp(`${String.fromCodePoint(27)}\\[[0-9;]*m`, 'g'), '');
}

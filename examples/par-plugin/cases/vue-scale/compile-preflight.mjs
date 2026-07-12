import { createRequire } from 'node:module';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import ts from 'typescript';
import {
  FROZEN_SELECTIONS,
  listUnexpectedPreparedFiles,
  readCorpusManifest,
  selectManifestEntries,
  summarizeSelection,
  verifyPreparedCorpus,
} from './corpus.mjs';
import {
  LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
  LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  LIFECYCLE_BASELINE_SOURCE_COMMIT,
  assertLocalExecution,
  assertRuntimeStable,
  inspectRuntimeProvenance,
} from './provenance.mjs';

assertLocalExecution();

const componentCount = Number(
  process.argv[2] ?? Math.max(...Object.keys(FROZEN_SELECTIONS).map(Number)),
);
if (!Object.hasOwn(FROZEN_SELECTIONS, componentCount)) {
  throw new Error(`compile preflight requires a frozen Vue scale, got ${process.argv[2]}`);
}
const outputPath = process.argv[3];
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const runtimePackageRoot = nodePath.resolve(
  process.argv[4] ?? nodePath.join(repositoryRoot, 'packages/rolldown'),
);
const runtimeRepositoryRoot = nodePath.resolve(runtimePackageRoot, '../..');
const runtimePin = {
  kind: 'lifecycle-corrected-baseline',
  sourceCommit: LIFECYCLE_BASELINE_SOURCE_COMMIT,
  nativeBindingSha256: LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  distributionSha256: LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
};
const runtime = await inspectRuntimeProvenance(runtimeRepositoryRoot, runtimePackageRoot, {
  requireClean: false,
  expectedPin: runtimePin,
});

const manifestPath = nodePath.join(import.meta.dirname, 'corpus-manifest.json');
const corpusDirectory = nodePath.join(import.meta.dirname, '.corpus');
const manifest = await readCorpusManifest(manifestPath);
await verifyPreparedCorpus({ corpusDirectory, manifest });
const unexpectedCorpusFiles = await listUnexpectedPreparedFiles(corpusDirectory, manifest);
if (unexpectedCorpusFiles.length !== 0) {
  throw new Error(`prepared Vue corpus has unexpected files: ${unexpectedCorpusFiles.join(', ')}`);
}
const selectedEntries = selectManifestEntries(manifest, componentCount);
const selectedIds = new Set(
  selectedEntries.map((entry) => nodePath.join(corpusDirectory, entry.sourceKey)),
);

process.env.ROLLDOWN_RESEARCH_PACKAGE_ROOT = runtimePackageRoot;
await import('./register-loader.mjs');
const [{ rolldown }, { vueTransformPlugin }] = await Promise.all([
  import('rolldown'),
  import('../../parallel-vue-plugin/impl.js'),
]);

const entryId = '\0vue-scale-compile-preflight-entry';
const exportHelperId = '\0/plugin-vue/export-helper';
const entryCode = `${selectedEntries
  .map(
    (entry, index) =>
      `export { default as component_${String(index).padStart(4, '0')} } from ${JSON.stringify(nodePath.join(corpusDirectory, entry.sourceKey))};`,
  )
  .join('\n')}\n`;
const virtualModules = {
  name: 'vue-scale-compile-preflight-virtual-modules',
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
let admitted = false;
let failures = [];
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
  await build.generate({ format: 'esm', sourcemap: true });
  admitted = true;
} catch (error) {
  if (!Array.isArray(error?.errors)) throw error;
  failures = await Promise.all(error.errors.map((failure) => classifyFailure(failure)));
  failures.sort((left, right) => compareUtf8(left.sourceKey, right.sourceKey));
} finally {
  await build?.close();
}

for (const failure of failures) {
  const id = nodePath.join(corpusDirectory, failure.sourceKey);
  if (!selectedIds.has(id)) {
    throw new Error(`compile preflight returned an unselected source: ${failure.sourceKey}`);
  }
}
await assertRuntimeStable(runtimeRepositoryRoot, runtimePackageRoot, runtime);

const report = {
  schema: 1,
  measurementClass: 'untimed compile admission; not performance evidence',
  runtime,
  corpus: {
    compiler: manifest.compiler,
    aggregateSha256: manifest.summary.aggregateSha256,
  },
  selection: summarizeSelection(selectedEntries),
  admitted,
  errorCount: failures.length,
  summary: summarizeFailures(failures),
  failures,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(
    JSON.stringify({
      outputPath,
      componentCount,
      admitted,
      errorCount: failures.length,
    }),
  );
} else {
  process.stdout.write(serialized);
}

async function classifyFailure(failure) {
  if (typeof failure?.id !== 'string') {
    throw new Error(`Vue compile failure omitted its source id: ${failure?.message}`);
  }
  const sourceKey = toPosixPath(nodePath.relative(corpusDirectory, failure.id));
  if (sourceKey.startsWith('../') || nodePath.isAbsolute(sourceKey)) {
    throw new Error(`Vue compile failure escaped the frozen corpus: ${failure.id}`);
  }
  const nested =
    Array.isArray(failure.errors) && failure.errors.length !== 0 ? failure.errors : [failure];
  if (nested.length !== 1) {
    throw new Error(`expected one nested transform failure for ${sourceKey}, got ${nested.length}`);
  }
  const diagnostic = nested[0];
  const message = stripAnsi(String(diagnostic?.message ?? failure.message ?? diagnostic));
  const code = diagnostic?.code ?? diagnostic?.kind ?? failure.code ?? 'UNKNOWN';
  const result = {
    sourceKey,
    repository: sourceKey.split('/')[0],
    code,
    signature: signatureFor(code, message),
  };
  if (code === 'TSCONFIG_ERROR') {
    const configPath = await findNearestTsconfig(failure.id);
    result.tsconfig = configPath
      ? await inspectTsconfig(configPath)
      : { root: null, unresolvedTargets: [{ kind: 'missing-root-config', target: null }] };
  } else if (message.startsWith('[@vue/compiler-sfc] Failed to resolve extends base type.')) {
    const unresolvedType = await inspectUnresolvedBaseType(message);
    if (unresolvedType) result.unresolvedType = unresolvedType;
  }
  return result;
}

function signatureFor(code, message) {
  if (code === 'TSCONFIG_ERROR' && message.includes('Tsconfig not found')) {
    return 'TSCONFIG_ERROR: missing dependency of nearest tsconfig';
  }
  if (message.startsWith('[@vue/compiler-sfc] Failed to resolve extends base type.')) {
    return 'compiler-sfc: unresolved extends base type';
  }
  return `${code}: ${message.split('\n')[0]}`;
}

async function findNearestTsconfig(sourcePath) {
  let directory = nodePath.dirname(sourcePath);
  while (directory.startsWith(corpusDirectory)) {
    const candidate = nodePath.join(directory, 'tsconfig.json');
    if (await exists(candidate)) return candidate;
    const parent = nodePath.dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
}

async function inspectTsconfig(rootPath) {
  const unresolvedTargets = [];
  const visited = new Set();
  async function visit(configPath) {
    const normalizedPath = nodePath.resolve(configPath);
    if (visited.has(normalizedPath)) return;
    visited.add(normalizedPath);
    const text = await readFile(normalizedPath, 'utf8');
    const parsed = ts.parseConfigFileTextToJson(normalizedPath, text);
    if (parsed.error || !parsed.config || typeof parsed.config !== 'object') {
      unresolvedTargets.push({
        kind: 'invalid-config',
        target: toCorpusPath(normalizedPath),
      });
      return;
    }
    const config = parsed.config;
    if (typeof config.extends === 'string') {
      const resolution = await resolveConfigTarget(normalizedPath, config.extends);
      if (resolution.path) await visit(resolution.path);
      else unresolvedTargets.push(resolution.failure);
    }
    if (Array.isArray(config.references)) {
      for (const reference of config.references) {
        if (typeof reference?.path !== 'string') continue;
        const resolution = await resolveReferencedConfig(normalizedPath, reference.path);
        if (resolution.path) await visit(resolution.path);
        else unresolvedTargets.push(resolution.failure);
      }
    }
  }
  await visit(rootPath);
  unresolvedTargets.sort((left, right) =>
    compareUtf8(`${left.kind}\0${left.target}`, `${right.kind}\0${right.target}`),
  );
  return {
    root: toCorpusPath(rootPath),
    unresolvedTargets,
  };
}

async function resolveConfigTarget(configPath, target) {
  if (target.startsWith('.')) {
    const base = nodePath.resolve(nodePath.dirname(configPath), target);
    for (const candidate of [base, `${base}.json`, nodePath.join(base, 'tsconfig.json')]) {
      if (await exists(candidate)) return { path: candidate };
    }
    return {
      failure: {
        kind: target.includes('.nuxt') ? 'generated-nuxt-config' : 'missing-relative-config',
        target: toCorpusPath(base),
      },
    };
  }
  try {
    return { path: createRequire(configPath).resolve(target) };
  } catch {
    return {
      failure: {
        kind: 'missing-package-config',
        target,
      },
    };
  }
}

async function resolveReferencedConfig(configPath, target) {
  const base = nodePath.resolve(nodePath.dirname(configPath), target);
  for (const candidate of [base, `${base}.json`, nodePath.join(base, 'tsconfig.json')]) {
    if (await exists(candidate)) return { path: candidate };
  }
  return {
    failure: {
      kind: 'missing-referenced-config',
      target: toCorpusPath(base),
    },
  };
}

async function inspectUnresolvedBaseType(message) {
  const location = message.match(/\n(\/[^\n]+\.(?:cts|mts|tsx?|d\.ts))\n/);
  if (!location) return;
  const sourcePath = location[1];
  const excerpt = message.slice(location.index + location[0].length);
  let currentLine;
  let diagnosticLine;
  for (const displayLine of excerpt.split('\n')) {
    const sourceLine = displayLine.match(/^\s*(\d+)\s*\|/);
    if (sourceLine) currentLine = Number(sourceLine[1]);
    if (/^\s*\|\s*\^/.test(displayLine) && currentLine) {
      diagnosticLine = currentLine;
      break;
    }
  }
  if (!diagnosticLine) return;
  const source = await readFile(sourcePath, 'utf8');
  const sourceFile = ts.createSourceFile(
    sourcePath,
    source,
    ts.ScriptTarget.Latest,
    true,
    sourcePath.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const imports = new Map();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) {
      continue;
    }
    const clause = statement.importClause;
    if (clause?.name) imports.set(clause.name.text, statement.moduleSpecifier.text);
    if (clause?.namedBindings && ts.isNamedImports(clause.namedBindings)) {
      for (const element of clause.namedBindings.elements) {
        imports.set(element.name.text, statement.moduleSpecifier.text);
      }
    }
  }
  const line = diagnosticLine - 1;
  let targetInterface;
  const visit = (node) => {
    if (ts.isInterfaceDeclaration(node)) {
      const start = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line;
      const end = sourceFile.getLineAndCharacterOfPosition(node.end).line;
      if (line >= start && line <= end) targetInterface = node;
    }
    if (!targetInterface) ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  if (!targetInterface) return;
  for (const clause of targetInterface.heritageClauses ?? []) {
    for (const type of clause.types) {
      let imported;
      const inspect = (node) => {
        if (!imported && ts.isIdentifier(node) && imports.has(node.text)) {
          imported = { symbol: node.text, module: imports.get(node.text) };
        }
        if (!imported) ts.forEachChild(node, inspect);
      };
      inspect(type);
      if (imported) {
        return {
          ...imported,
          definition: toCorpusPath(sourcePath),
          modulePresent: canResolveFrom(sourcePath, imported.module),
        };
      }
    }
  }
}

function canResolveFrom(sourcePath, specifier) {
  try {
    createRequire(sourcePath).resolve(specifier);
    return true;
  } catch {
    return false;
  }
}

function summarizeFailures(values) {
  const byRepository = countBy(values, (failure) => failure.repository);
  const bySignature = countBy(values, (failure) => failure.signature);
  const byTsconfigRoot = countBy(
    values.filter((failure) => failure.tsconfig),
    (failure) => failure.tsconfig.root ?? '<missing-root-config>',
  );
  const unresolvedTsconfigTargets = new Map();
  for (const failure of values) {
    for (const target of failure.tsconfig?.unresolvedTargets ?? []) {
      const key = `${target.kind}\0${target.target ?? '<none>'}`;
      const previous = unresolvedTsconfigTargets.get(key) ?? { ...target, affectedFailures: 0 };
      previous.affectedFailures++;
      unresolvedTsconfigTargets.set(key, previous);
    }
  }
  const byUnresolvedTypeModule = countBy(
    values.filter((failure) => failure.unresolvedType),
    (failure) => failure.unresolvedType.module,
  );
  return {
    byRepository,
    bySignature,
    byTsconfigRoot,
    unresolvedTsconfigTargets: [...unresolvedTsconfigTargets.values()].sort((left, right) =>
      compareUtf8(`${left.kind}\0${left.target}`, `${right.kind}\0${right.target}`),
    ),
    byUnresolvedTypeModule,
  };
}

function countBy(values, keyFor) {
  const counts = new Map();
  for (const value of values) {
    const key = keyFor(value);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return Object.fromEntries([...counts].sort(([left], [right]) => compareUtf8(left, right)));
}

function toCorpusPath(path) {
  const relative = toPosixPath(nodePath.relative(corpusDirectory, path));
  return relative.startsWith('../') ? path : relative;
}

function toPosixPath(value) {
  return value.split(nodePath.sep).join('/');
}

function stripAnsi(value) {
  const ansiEscape = String.fromCodePoint(27);
  return value.replaceAll(new RegExp(`${ansiEscape}\\[[0-9;]*m`, 'g'), '');
}

function compareUtf8(left, right) {
  return Buffer.compare(Buffer.from(left), Buffer.from(right));
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
}

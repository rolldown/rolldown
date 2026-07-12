import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { parse as parseVueSfc } from '@vue/compiler-sfc';
import { createGraphSupport, createInputs } from './graph-support.mjs';
import { ensurePreparedProject } from './prepare-projects.mjs';
import { assertLocalNode, projectDefinition } from './projects.mjs';

assertLocalNode();
const projectId = process.argv[2];
const traversalMode = process.argv[3] ?? 'stub';
if (traversalMode !== 'stub' && traversalMode !== 'script-closure') {
  throw new Error(`unknown reachability traversal mode: ${traversalMode}`);
}
const baseProject = projectDefinition(projectId);
const entryOverride = process.argv[4];
const appSourceRootOverride = process.argv[5];
const project = entryOverride
  ? {
      ...baseProject,
      entries: [entryOverride],
      appSourceRoot: appSourceRootOverride,
      minimumReachedSfcCount: 0,
    }
  : baseProject;
if (
  !project.protocolStatus?.includes('Reachability-only') &&
  !(entryOverride && projectId === 'vben')
) {
  throw new Error('run-reachability.mjs is restricted to explicit amendment candidates');
}

const prepared = await ensurePreparedProject(projectId);
const root = prepared.root;
const { input, virtualEntries, entryProvenance } = await createInputs(project, root);
const graphSupport = await createGraphSupport(project, root, virtualEntries);
const reached = new Map();
const sfcScopeRoots = project.sfcRoots.map((relative) => nodePath.resolve(root, relative));
const isInSfcScope = (path) =>
  sfcScopeRoots.some((scopeRoot) => {
    const relative = nodePath.relative(scopeRoot, path);
    return relative === '' || (!relative.startsWith('..') && !nodePath.isAbsolute(relative));
  });
const moduleIds = new Set();
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const portable = (path) => path.split(nodePath.sep).join('/');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sfcStub = {
  name: 'independent-vue-reachability-sfc-stub',
  async load(id) {
    if (!id.endsWith('.vue') || !nodePath.isAbsolute(id)) return;
    const content = await readFile(id);
    if (isInSfcScope(id)) {
      reached.set(id, { bytes: content.byteLength, sha256: sha256(content) });
    }
    if (traversalMode === 'stub') return { code: 'export default {};', moduleType: 'js' };
    const parsed = parseVueSfc(content.toString('utf8'), { filename: id });
    if (parsed.errors.length !== 0) {
      throw new Error(`failed to parse reached SFC ${id}: ${parsed.errors.join('; ')}`);
    }
    const blocks = [parsed.descriptor.script, parsed.descriptor.scriptSetup].filter(Boolean);
    const externalBlocks = blocks
      .filter((block) => block.src)
      .map((block) => `import ${JSON.stringify(block.src)};`);
    const specifiers = new Set();
    for (const block of blocks) {
      for (const match of block.content.matchAll(
        /(?:\b(?:import|export)\s+(?:[\s\S]*?\s+from\s+)?|\bimport\s*\(\s*)['"]([^'"]+)['"]/g,
      )) {
        specifiers.add(match[1]);
      }
    }
    const imports = [...specifiers]
      .sort(byteSort)
      .map((source) => `import ${JSON.stringify(source)};`);
    return {
      code: [...externalBlocks, ...imports, 'export default {};'].join('\n'),
      moduleType: 'js',
    };
  },
  moduleParsed(info) {
    moduleIds.add(info.id);
  },
};

let build;
try {
  process.chdir(root);
  build = await rolldown({
    cwd: root,
    input,
    logLevel: 'silent',
    resolve: { symlinks: false },
    tsconfig: false,
    treeshake: true,
    plugins: [sfcStub, graphSupport.plugin],
  });
  const output = await build.generate({ format: 'esm' });
  await build.close();
  build = undefined;
  const paths = [...reached.keys()].sort((left, right) =>
    byteSort(portable(nodePath.relative(root, left)), portable(nodePath.relative(root, right))),
  );
  const manifest = createHash('sha256');
  let bytes = 0;
  for (const path of paths) {
    const value = reached.get(path);
    const relative = portable(nodePath.relative(root, path));
    manifest.update(`${relative}\0${value.bytes}\0${value.sha256}\n`);
    bytes += value.bytes;
  }
  const normalizedModules = [...moduleIds]
    .map((id) => id.replaceAll(root, '<project-root>'))
    .sort(byteSort);
  console.log(
    JSON.stringify({
      schema: 1,
      projectId,
      measurementClass: 'static-entry-graph-lower-bound',
      ordinaryVueTransformCorrectness: false,
      traversalMode,
      reason:
        traversalMode === 'stub'
          ? 'Each reached SFC is replaced at load with an empty JavaScript module, so imports inside SFC script blocks are not traversed. All entry, JS/TS, alias, relative, and expanded import.meta.glob edges before each SFC are real.'
          : 'Reached SFC script and script-setup blocks are parsed, and every statically recognizable import/export-from/dynamic-import specifier is converted to a side-effect import. Type-only imports are retained, so this is a conservative script-edge closure rather than Vue transform correctness.',
      prepared,
      entryProvenance,
      reachedSfc: {
        count: paths.length,
        bytes,
        manifestSha256: manifest.digest('hex'),
        paths: paths.map((path) => portable(nodePath.relative(root, path))),
      },
      physicalSfcCount: project.expectedPhysicalSfcCount,
      minimumCandidateSfcCount: project.minimumReachedSfcCount,
      lowerBoundMeetsMinimum: paths.length >= project.minimumReachedSfcCount,
      graph: {
        moduleCount: normalizedModules.length,
        moduleManifestSha256: sha256(`${normalizedModules.join('\n')}\n`),
        support: graphSupport.report(),
      },
      output: {
        count: output.output.length,
        chunks: output.output.filter((item) => item.type === 'chunk').length,
        assets: output.output.filter((item) => item.type === 'asset').length,
      },
    }),
  );
} catch (error) {
  console.log(
    JSON.stringify({
      schema: 1,
      projectId,
      measurementClass: 'static-entry-graph-lower-bound',
      traversalMode,
      executionStatus: 'failed',
      partialReachedSfcCount: reached.size,
      error: {
        name: error.name,
        message: String(error.message).replaceAll(root, '<project-root>'),
        stackSha256: sha256(String(error.stack).replaceAll(root, '<project-root>')),
      },
    }),
  );
  process.exitCode = 2;
} finally {
  await build?.close();
}

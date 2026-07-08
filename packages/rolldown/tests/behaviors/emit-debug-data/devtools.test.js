import { rolldown } from 'rolldown';

import { existsSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';

// `.rolldown` dir is generated based on real cwd instead of `InputOptions.cwd`. We might be able to solve this in the future.
// For now, we just live with it.
const dotRolldownFileName = join(process.cwd(), 'node_modules/.rolldown');

function normalizePath(path) {
  return path.replaceAll('\\', '/');
}

function expectPathToEndWith(path, suffix) {
  expect(normalizePath(path).endsWith(suffix)).toBe(true);
}

function cleanDebugData() {
  if (existsSync(dotRolldownFileName)) {
    rmSync(dotRolldownFileName, { recursive: true, force: true });
  }
}

function readDebugLogs() {
  const sessions = readdirSync(dotRolldownFileName);
  expect(sessions).toHaveLength(1);
  return readFileSync(join(dotRolldownFileName, sessions[0], 'logs.json'), 'utf8')
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line));
}

function findByModuleSuffix(items, field, suffix) {
  return items.find((item) => normalizePath(item[field]).endsWith(suffix));
}

async function runStrictOrderBundle({ strictExecutionOrder, input, trace = true }) {
  cleanDebugData();
  const previousTraceValue = process.env.ROLLDOWN_STRICT_ORDER_TRACE;
  if (trace) {
    process.env.ROLLDOWN_STRICT_ORDER_TRACE = '1';
  } else {
    delete process.env.ROLLDOWN_STRICT_ORDER_TRACE;
  }
  const bundle = await rolldown({
    cwd: import.meta.dirname,
    devtools: {},
    input,
    plugins: [
      {
        name: 'strict-order-render-marker',
        renderChunk(code) {
          return code;
        },
      },
    ],
  });
  const { output } = await bundle.generate({ strictExecutionOrder });
  await bundle.close();
  if (previousTraceValue === undefined) {
    delete process.env.ROLLDOWN_STRICT_ORDER_TRACE;
  } else {
    process.env.ROLLDOWN_STRICT_ORDER_TRACE = previousTraceValue;
  }
  return { logs: readDebugLogs(), output };
}

test(`emit data for devtool`, async () => {
  // Clean up previous test data if exists
  cleanDebugData();

  const renderedModuleSizes = await runBundle();

  const dotRolldownDir = readdirSync(dotRolldownFileName);
  expect(dotRolldownDir.length).toBe(1);
  const debugDataDir = readdirSync(join(dotRolldownFileName, dotRolldownDir[0]));
  // Expect `logs.json` and `meta.json` exist
  expect(debugDataDir).toContain('logs.json');
  expect(debugDataDir).toContain('meta.json');

  // Ensure there are no invalid uninjected variables in the logs
  const variables = ['${build_id}', '${session_id}', '${hook_resolve_id_trigger}'];
  const logsContent = readFileSync(join(dotRolldownFileName, dotRolldownDir[0], 'logs.json'));
  for (const variable of variables) {
    expect(logsContent.includes(variable)).toBe(false);
  }

  const logs = logsContent
    .toString()
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line));

  const moduleGraphReady = logs.find((event) => event.action === 'ModuleGraphReady');
  expect(moduleGraphReady).toBeDefined();
  const entryModule = moduleGraphReady.modules.find((module) => /[\\/]index\.ts$/.test(module.id));
  expect(entryModule).toBeDefined();
  expect(entryModule.imports).toContainEqual(
    expect.objectContaining({
      kind: 'dynamic-import',
      module_request: './async',
    }),
  );

  const chunkGraphReady = logs.find((event) => event.action === 'ChunkGraphReady');
  expect(chunkGraphReady).toBeDefined();
  expect(
    chunkGraphReady.chunks.some((chunk) =>
      chunk.imports.some((item) => item.kind === 'dynamic-import'),
    ),
  ).toBe(true);
  const chunkById = new Map(chunkGraphReady.chunks.map((chunk) => [chunk.chunk_id, chunk]));

  const packageGraphReady = logs.find((event) => event.action === 'PackageGraphReady');
  expect(packageGraphReady).toBeDefined();

  function expectPackageSize(pkg) {
    expect(Number.isInteger(pkg.size)).toBe(true);
    expect(pkg.size).toBeGreaterThanOrEqual(0);

    let expectedSize = 0;
    for (const moduleId of pkg.modules) {
      const moduleSize = renderedModuleSizes.get(moduleId);
      expect(moduleSize).toBeDefined();
      expectedSize += moduleSize;
    }
    expect(pkg.size).toBe(expectedSize);
  }

  function expectPackageChunkLinks(pkg) {
    expect(['direct', 'transitive']).toContain(pkg.dependency_type);
    expect(pkg.modules).toEqual([...new Set(pkg.modules)]);
    expect(pkg.chunk_ids).toEqual([...new Set(pkg.chunk_ids)]);

    if (pkg.is_used) {
      expect(pkg.modules.length).toBeGreaterThan(0);
      expect(pkg.chunk_ids.length).toBeGreaterThan(0);
    }

    for (const chunkId of pkg.chunk_ids) {
      const chunk = chunkById.get(chunkId);
      expect(chunk).toBeDefined();
      expect(pkg.modules.some((moduleId) => chunk.modules.includes(moduleId))).toBe(true);
    }
  }

  for (const pkg of packageGraphReady.packages) {
    expectPackageChunkLinks(pkg);
    expectPackageSize(pkg);
  }

  const metaInfoPackage = packageGraphReady.packages.find((pkg) => pkg.name === 'meta-info-lib');
  expect(metaInfoPackage).toEqual(
    expect.objectContaining({
      dependency_type: 'direct',
      is_used: true,
      version: '1.2.3',
    }),
  );
  expect(metaInfoPackage.package_id).toBe(metaInfoPackage.package_root);
  expectPathToEndWith(metaInfoPackage.package_root, 'node_modules/meta-info-lib');
  expectPathToEndWith(metaInfoPackage.package_json_path, 'node_modules/meta-info-lib/package.json');
  expect(metaInfoPackage.size).toBeGreaterThan(0);
  expect(metaInfoPackage.modules).toHaveLength(1);
  expectPathToEndWith(metaInfoPackage.modules[0], 'node_modules/meta-info-lib/index.js');

  const duplicatePackages = packageGraphReady.packages.filter(
    (pkg) => pkg.name === 'duplicate-lib',
  );
  expect(duplicatePackages).toHaveLength(2);
  expect(duplicatePackages.map((pkg) => pkg.version)).toEqual(['1.0.0', '2.0.0']);
  expect(duplicatePackages.every((pkg) => pkg.dependency_type === 'direct')).toBe(true);
  expect(duplicatePackages.every((pkg) => pkg.is_used)).toBe(true);
  expect(duplicatePackages.every((pkg) => pkg.size > 0)).toBe(true);
  expect(new Set(duplicatePackages.map((pkg) => pkg.package_root)).size).toBe(2);
  expectPathToEndWith(duplicatePackages[0].package_root, 'node_modules/duplicate-a');
  expectPathToEndWith(duplicatePackages[1].package_root, 'node_modules/duplicate-b');
  expect(duplicatePackages[0].modules).toHaveLength(1);
  expectPathToEndWith(duplicatePackages[0].modules[0], 'node_modules/duplicate-a/index.js');
  expect(duplicatePackages[1].modules).toHaveLength(1);
  expectPathToEndWith(duplicatePackages[1].modules[0], 'node_modules/duplicate-b/index.js');

  const duplicatePackageIndices = packageGraphReady.packages.flatMap((pkg, index) =>
    pkg.name === 'duplicate-lib' ? [index] : [],
  );
  expect(duplicatePackageIndices[1]).toBe(duplicatePackageIndices[0] + 1);

  const directGraphPackage = packageGraphReady.packages.find(
    (pkg) => pkg.name === 'direct-graph-lib',
  );
  expect(directGraphPackage).toEqual(
    expect.objectContaining({
      dependency_type: 'direct',
      is_used: true,
      version: '1.0.0',
    }),
  );
  expectPathToEndWith(directGraphPackage.package_root, 'node_modules/direct-graph-lib');
  expect(directGraphPackage.size).toBeGreaterThan(0);
  expect(directGraphPackage.modules).toHaveLength(1);
  expectPathToEndWith(directGraphPackage.modules[0], 'node_modules/direct-graph-lib/index.js');

  const transitiveGraphPackage = packageGraphReady.packages.find(
    (pkg) => pkg.name === 'transitive-graph-lib',
  );
  expect(transitiveGraphPackage).toEqual(
    expect.objectContaining({
      dependency_type: 'transitive',
      is_used: true,
      version: '1.0.0',
    }),
  );
  expectPathToEndWith(transitiveGraphPackage.package_root, 'node_modules/transitive-graph-lib');
  expect(transitiveGraphPackage.size).toBeGreaterThan(0);
  expect(transitiveGraphPackage.modules).toHaveLength(1);
  expectPathToEndWith(
    transitiveGraphPackage.modules[0],
    'node_modules/transitive-graph-lib/index.js',
  );

  const unusedPackage = packageGraphReady.packages.find((pkg) => pkg.name === 'unused-lib');
  expect(unusedPackage).toEqual(
    expect.objectContaining({
      dependency_type: 'direct',
      is_used: false,
      version: '1.0.0',
    }),
  );
  expectPathToEndWith(unusedPackage.package_root, 'node_modules/unused-lib');
  expectPathToEndWith(unusedPackage.package_json_path, 'node_modules/unused-lib/package.json');
  expect(unusedPackage.size).toBe(0);
  expect(unusedPackage.modules).toEqual([]);
  expect(unusedPackage.chunk_ids).toEqual([]);

  const metaContent = readFileSync(join(dotRolldownFileName, dotRolldownDir[0], 'meta.json'));
  for (const variable of variables) {
    expect(metaContent.includes(variable)).toBe(false);
  }

  async function runBundle() {
    const bundle = await rolldown({
      devtools: {},
      cwd: import.meta.dirname,
      input: join(import.meta.dirname, 'index.ts'),
      resolve: {
        // This needs to be explicitly set for now because oxc resolver doesn't
        // assume default exports conditions. Rolldown will ship with a default that
        // aligns with Vite in the future.
        conditionNames: ['import'],
      },
      plugins: [
        {
          name: 'test',
          renderChunk(code) {
            return code + '\n// test';
          },
        },
        {
          name: 'test-resolve',
          async resolveId(source, importer) {
            // Test this.resolve functionality
            if (source === './hello' && importer) {
              console.log(`[test-resolve] Resolving "${source}" from "${importer}"`);

              // Use this.resolve to resolve the module
              const resolved = await this.resolve('./hello.ts', importer, {
                skipSelf: true, // Skip this plugin to avoid infinite recursion
              });

              if (resolved) {
                console.log(`[test-resolve] Successfully resolved to: ${resolved.id}`);
                return resolved;
              } else {
                console.log(`[test-resolve] Failed to resolve "${source}"`);
              }
            }
            return null;
          },
        },
      ],
    });
    const { output } = await bundle.generate();
    const renderedModuleSizes = new Map();
    for (const item of output) {
      if (item.type !== 'chunk') {
        continue;
      }

      for (const [moduleId, module] of Object.entries(item.modules)) {
        renderedModuleSizes.set(
          moduleId,
          (renderedModuleSizes.get(moduleId) ?? 0) + Buffer.byteLength(module.code ?? ''),
        );
      }
    }
    // Devtools log files are only guaranteed complete after `close()` — the
    // writer thread drains and flushes on the CloseSession ack.
    await bundle.close();
    return renderedModuleSizes;
  }
});

test('emit the strict execution order plan after final topology is available', async () => {
  const fixtureDir = join(import.meta.dirname, 'strict-order');
  const { logs, output } = await runStrictOrderBundle({
    strictExecutionOrder: true,
    input: {
      main: join(fixtureDir, 'main.js'),
      second: join(fixtureDir, 'second.js'),
    },
  });

  const action = logs.find((event) => event.action === 'StrictExecutionOrderPlanReady');
  expect(action).toBeDefined();
  expect(action.version).toBe(2);
  expect(logs.indexOf(action)).toBeLessThan(
    logs.findIndex((event) => event.action === 'HookRenderChunkStart'),
  );

  const pPlan = findByModuleSuffix(action.plan_modules, 'module_id', '/strict-order/p.js');
  expect(pPlan.reasons).toEqual(expect.arrayContaining(['direct-violation', 'sensitive-suffix']));
  const mainPlan = findByModuleSuffix(action.plan_modules, 'module_id', '/strict-order/main.js');
  expect(mainPlan.reasons).toEqual(
    expect.arrayContaining(['sensitive-suffix', 'static-importer', 'top-level-reader']),
  );

  const mainRoot = findByModuleSuffix(action.roots, 'root_module_id', '/strict-order/main.js');
  const expectedOrder = mainRoot.expected_order.map(normalizePath);
  const predictedOrder = mainRoot.predicted_pre_wrap_order.map(normalizePath);
  const expectedEIndex = expectedOrder.findIndex((id) => id.endsWith('/strict-order/e.js'));
  const expectedPIndex = expectedOrder.findIndex((id) => id.endsWith('/strict-order/p.js'));
  const predictedEIndex = predictedOrder.findIndex((id) => id.endsWith('/strict-order/e.js'));
  const predictedPIndex = predictedOrder.findIndex((id) => id.endsWith('/strict-order/p.js'));
  expect(expectedEIndex).toBeLessThan(expectedPIndex);
  expect(predictedPIndex).toBeLessThan(predictedEIndex);
  expect(
    mainRoot.at_risk_modules.some((id) => normalizePath(id).endsWith('/strict-order/p.js')),
  ).toBe(true);

  const pModule = findByModuleSuffix(action.included_modules, 'module_id', '/strict-order/p.js');
  const mainModule = findByModuleSuffix(
    action.included_modules,
    'module_id',
    '/strict-order/main.js',
  );
  const dynamicModule = findByModuleSuffix(
    action.included_modules,
    'module_id',
    '/strict-order/dynamic.js',
  );
  expect(pModule).toEqual(
    expect.objectContaining({
      interop_wrap_kind: 'none',
      order_wrapped: true,
      wrapper_origin: 'execution-order',
      wrapper_included: true,
      tla_tainted: true,
    }),
  );
  expect(mainModule.final_chunk_id).not.toBeNull();
  expect(mainModule.entry_chunk_id).not.toBeNull();
  expect(mainModule.entry_trigger).toBe('order-init');

  const chunksById = new Map(action.rendered_chunks.map((chunk) => [chunk.chunk_id, chunk]));
  const mainEntryChunk = chunksById.get(mainModule.entry_chunk_id);
  expect(mainEntryChunk.static_chunk_imports).toContain(mainModule.final_chunk_id);
  const mainFinalChunk = chunksById.get(mainModule.final_chunk_id);
  expect(mainFinalChunk.static_chunk_imports).toContain(pModule.final_chunk_id);
  expect(
    action.rendered_chunks.some((chunk) =>
      chunk.dynamic_chunk_imports.includes(dynamicModule.final_chunk_id),
    ),
  ).toBe(true);

  const directPObligations = action.init_obligations.filter(
    (obligation) =>
      obligation.kind === 'direct-import' &&
      normalizePath(obligation.importee_id).endsWith('/strict-order/p.js'),
  );
  expect(directPObligations).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        awaited: true,
        importer_tla_tainted: true,
        importee_tla_tainted: true,
      }),
    ]),
  );
  expect(directPObligations.map((obligation) => normalizePath(obligation.importer_id))).toEqual([
    expect.stringMatching(/\/strict-order\/main\.js$/),
  ]);
  expect(
    action.init_obligations.some(
      (obligation) =>
        obligation.kind === 'transitive-init-target' &&
        normalizePath(obligation.importer_id).endsWith('/strict-order/second.js') &&
        normalizePath(obligation.importee_id).endsWith('/strict-order/p.js') &&
        obligation.awaited === false,
    ),
  ).toBe(true);

  const wrappedOwner = findByModuleSuffix(
    action.included_modules,
    'module_id',
    '/strict-order/wrapped-owner.js',
  );
  expect(
    output
      .filter((item) => item.type === 'chunk')
      .map((chunk) => chunk.code)
      .join('\n'),
  ).toMatch(/await init_wrapped_owner\d*\(\)/);
  expect(
    action.init_obligations.some(
      (obligation) =>
        obligation.kind === 'direct-import' &&
        normalizePath(obligation.importer_id).endsWith('/strict-order/main.js') &&
        obligation.importee_id === wrappedOwner.module_id &&
        obligation.awaited === true &&
        obligation.importee_tla_tainted === true,
    ),
  ).toBe(true);

  const includedForwardingBarrel = findByModuleSuffix(
    action.included_modules,
    'module_id',
    '/strict-order/included-forwarding-barrel.js',
  );
  const interopOwner = findByModuleSuffix(
    action.included_modules,
    'module_id',
    '/strict-order/interop-owner.js',
  );
  expect(includedForwardingBarrel).toEqual(
    expect.objectContaining({
      final_chunk_id: mainModule.final_chunk_id,
      interop_wrap_kind: 'none',
      order_wrapped: false,
      wrapper_origin: 'none',
    }),
  );
  expect(interopOwner).toEqual(
    expect.objectContaining({
      interop_wrap_kind: 'esm',
      order_wrapped: false,
      wrapper_origin: 'interop-esm',
      wrapper_included: true,
    }),
  );
  expect(
    output
      .filter((item) => item.type === 'chunk')
      .map((chunk) => chunk.code)
      .join('\n'),
  ).toMatch(/init_interop_owner\d*\(\)/);
  expect(
    action.init_obligations.some(
      (obligation) =>
        obligation.kind === 'direct-import' &&
        obligation.importer_id === mainModule.module_id &&
        obligation.importee_id === interopOwner.module_id,
    ),
  ).toBe(false);
  expect(
    action.init_obligations.some(
      (obligation) =>
        obligation.kind === 'direct-import' &&
        obligation.importer_id === includedForwardingBarrel.module_id &&
        obligation.importee_id === interopOwner.module_id &&
        obligation.awaited === false,
    ),
  ).toBe(true);
});

test('do not emit the strict execution order plan when strict mode is disabled', async () => {
  const fixtureDir = join(import.meta.dirname, 'strict-order');
  const { logs } = await runStrictOrderBundle({
    strictExecutionOrder: false,
    input: {
      main: join(fixtureDir, 'main.js'),
      second: join(fixtureDir, 'second.js'),
    },
  });

  expect(logs.some((event) => event.action === 'StrictExecutionOrderPlanReady')).toBe(false);
});

test('do not emit the strict execution order plan without the explicit trace opt-in', async () => {
  const { logs } = await runStrictOrderBundle({
    strictExecutionOrder: true,
    trace: false,
    input: join(import.meta.dirname, 'strict-order/empty.js'),
  });

  expect(logs.some((event) => event.action === 'StrictExecutionOrderPlanReady')).toBe(false);
});

test('emit an empty strict execution order plan when analysis ran without a hazard', async () => {
  const { logs } = await runStrictOrderBundle({
    strictExecutionOrder: true,
    input: join(import.meta.dirname, 'strict-order/empty.js'),
  });

  const action = logs.find((event) => event.action === 'StrictExecutionOrderPlanReady');
  expect(action).toEqual(
    expect.objectContaining({
      action: 'StrictExecutionOrderPlanReady',
      version: 2,
      plan_modules: [],
    }),
  );
  expect(action.roots).toHaveLength(1);
});

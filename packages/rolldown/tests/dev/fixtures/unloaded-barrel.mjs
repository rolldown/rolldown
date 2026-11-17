import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import vm from 'node:vm';
import { dev } from 'rolldown/experimental';
import { DevRuntime } from 'rolldown/experimental/runtime';

const scenario = process.argv[2];
const lazy = scenario === 'lazy';
const temp = fileURLToPath(new URL('../temp/', import.meta.url));
fs.mkdirSync(temp, { recursive: true });
const root = fs.mkdtempSync(path.join(temp, 'unloaded-barrel-'));
const view = (value) => `import { format } from './barrel.js';
export const render = () => format(${JSON.stringify(value)});
globalThis.rendered = render();
if (import.meta.hot) import.meta.hot.accept();
`;
const files = {
  'main.js': `import { render } from './view.js';
globalThis.rendered = render();
${lazy ? "globalThis.openRoute = () => import('./lazy.js');" : ''}
`,
  'view.js': view('before'),
  'barrel.js':
    scenario === 'extra'
      ? `import { format } from './implementation.js';
export { format };
export const extra = format('extra');
`
      : "export * from './implementation.js';\n",
  'implementation.js': "export const format = value => 'rendered:' + value;\n",
  'lazy.js': "import { format } from './barrel.js';\nexport const value = format('lazy');\n",
};
for (const [name, code] of Object.entries(files)) {
  fs.writeFileSync(path.join(root, name), code);
}
function runSource() {
  const mainUrl = pathToFileURL(path.join(root, 'main.js')).href;
  const barrelUrl = pathToFileURL(path.join(root, 'barrel.js')).href;
  const code = `await import(${JSON.stringify(mainUrl)});
const barrel = await import(${JSON.stringify(barrelUrl)});
console.log(JSON.stringify({ rendered: globalThis.rendered, extra: barrel.extra,
  ${lazy ? 'lazy: (await globalThis.openRoute()).value' : ''} }));`;
  return JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', code], { encoding: 'utf8' }),
  );
}

let engine;
let output;
let resolveUpdate;
let rejectUpdate;
let timeout;
const update = new Promise((resolve, reject) => {
  resolveUpdate = resolve;
  rejectUpdate = reject;
});
const runtime = new DevRuntime('test-client');
runtime.hooks = { createModuleHotContext: () => ({ accept() {} }), onModuleCacheRemoval() {} };
const context = vm.createContext({ __rolldown_runtime__: runtime });
runtime.loadLazyChunk = async (proxyId) => {
  const chunk = await engine.compileEntry(proxyId, runtime.clientId);
  vm.runInContext(chunk.code, context);
};
try {
  const before = runSource();
  engine = await dev(
    {
      cwd: root,
      input: path.join(root, 'main.js'),
      experimental: { devMode: { lazy, implement: '', skipCommonRuntimeInjection: true } },
      optimization: { inlineConst: false },
      plugins: [
        {
          name: 'side-effect-free-barrel',
          resolveId(source) {
            if (source === './barrel.js')
              return { id: path.join(root, 'barrel.js'), moduleSideEffects: false };
          },
        },
      ],
    },
    {
      format: 'es',
      entryFileNames: 'main.js',
      codeSplitting: {
        groups: [
          {
            name: 'barrel',
            test: /barrel\.js$/,
            includeDependenciesRecursively: false,
            entriesAware: false,
            minShareCount: 1,
          },
        ],
      },
    },
    {
      watch: { ...getDevWatchOptionsForCi(), enabled: !lazy, skipWrite: true },
      rebuildStrategy: 'never',
      onOutput(result) {
        output ??= result;
      },
      onHmrUpdates(result) {
        if (result instanceof Error) {
          rejectUpdate(result);
          return;
        }
        const patch = result.updates.find(
          ({ clientId, update }) => clientId === runtime.clientId && update.type === 'Patch',
        );
        if (patch) resolveUpdate(patch.update);
      },
    },
  );
  await engine.run();
  if (output instanceof Error) throw output;
  const modules = new Map(
    output.output
      .filter((item) => item.type === 'chunk')
      .map((chunk) => [
        chunk.fileName,
        new vm.SourceTextModule(chunk.code, { context, identifier: chunk.fileName }),
      ]),
  );
  const entry = modules.get('main.js');
  await entry.link((specifier, parent) =>
    modules.get(path.posix.join(path.posix.dirname(parent.identifier), specifier)),
  );
  await entry.evaluate();
  assert.equal(context.rendered, before.rendered);
  assert.equal(runtime.isExecuted('barrel.js'), false);
  const result = { initial: context.rendered };
  await engine.registerClient(runtime.clientId);
  if (lazy) {
    result.lazy = (await context.openRoute()).value;
    assert.equal(result.lazy, before.lazy);
  } else {
    timeout = setTimeout(() => rejectUpdate(new Error('HMR update timed out')), 10_000);
    fs.writeFileSync(path.join(root, 'view.js'), view('after'));
    const after = runSource();
    const patch = await update;
    vm.runInContext(patch.code, context);
    runtime.removeModuleCache('view.js');
    runtime.initModule('view.js');
    result.updated = context.rendered;
    assert.equal(result.updated, after.rendered);
    if (scenario === 'extra') {
      result.extra = runtime.loadExports('barrel.js').extra;
      assert.equal(result.extra, after.extra);
    }
  }
  console.log(JSON.stringify(result));
} finally {
  clearTimeout(timeout);
  await engine?.close();
  fs.rmSync(root, { recursive: true, force: true });
}

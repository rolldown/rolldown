import { realpathSync } from 'node:fs';
import nodePath from 'node:path';
import { compile, compileModule, VERSION } from 'svelte/compiler';
import ts from 'typescript';
import { COUNTER, MAX_WORKERS } from './metrics.js';

export const SVELTE_VERSION = VERSION;

const updateMax = (counters, index, candidate) => {
  let current = Atomics.load(counters, index);
  while (candidate > current) {
    const previous = Atomics.compareExchange(counters, index, current, candidate);
    if (previous === current) return;
    current = previous;
  }
};

const normalizeWarning = (warning, id) => ({
  code: `SVELTE_${warning.code.toUpperCase()}`,
  message: warning.message,
  id,
  loc: warning.start
    ? { file: id, line: warning.start.line, column: warning.start.column }
    : undefined,
  frame: warning.frame,
});

const stripTypeScript = (code, filename) => {
  const result = ts.transpileModule(code, {
    fileName: filename,
    reportDiagnostics: true,
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ESNext,
      sourceMap: true,
      inlineSources: true,
      verbatimModuleSyntax: true,
    },
  });
  const errors = result.diagnostics?.filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  );
  if (errors?.length) {
    throw new Error(
      errors
        .map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))
        .join('\n'),
    );
  }
  return {
    code: result.outputText.replace(/\n?\/\/# sourceMappingURL=.*\n?$/, '\n'),
    map: JSON.parse(result.sourceMapText),
  };
};

const compileSource = (code, id, corpusDirectory, context) => {
  const filename = nodePath.relative(corpusDirectory, id).split(nodePath.sep).join('/');
  const module = /\.svelte\.(?:js|ts)$/.test(id);
  const prepared = module ? stripTypeScript(code, filename) : undefined;
  const result = module
    ? compileModule(prepared.code, {
        filename,
        generate: 'client',
        dev: false,
        discloseVersion: false,
        sourcemap: prepared.map,
      })
    : compile(code, {
        filename,
        generate: 'client',
        dev: false,
        css: 'injected',
        discloseVersion: false,
      });
  for (const warning of result.warnings) context.warn(normalizeWarning(warning, id));
  const map = JSON.parse(result.js.map.toString());
  return {
    module,
    transformResult: { code: result.js.code, map },
    mapBytes: Buffer.byteLength(JSON.stringify(map)),
    warningCount: result.warnings.length,
  };
};

export const createSvelteRegistryGraphPlugin = (options, threadNumber = 0) => {
  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid Svelte registry graph thread number: ${threadNumber}`);
  }
  const corpusDirectory = realpathSync(options.corpusDirectory);
  const counters = options.metricsBuffer ? new BigInt64Array(options.metricsBuffer) : undefined;
  if (counters) {
    Atomics.add(counters, COUNTER.factoryCalls, 1n);
    Atomics.or(counters, COUNTER.workerMask, 1n << BigInt(threadNumber));
  }

  if (!counters) {
    return {
      name: 'svelte-registry-graph-transform',
      transform: {
        filter: { id: { include: [/\.svelte(?:\.(?:js|ts))?$/] } },
        handler(code, id) {
          return compileSource(code, id, corpusDirectory, this).transformResult;
        },
      },
    };
  }

  return {
    name: 'svelte-registry-graph-transform',
    transform: {
      filter: { id: { include: [/\.svelte(?:\.(?:js|ts))?$/] } },
      handler(code, id) {
        const startedAt = process.hrtime.bigint();
        const module = /\.svelte\.(?:js|ts)$/.test(id);
        const active = Atomics.add(counters, COUNTER.active, 1n) + 1n;
        updateMax(counters, COUNTER.maxActive, active);
        Atomics.add(counters, module ? COUNTER.moduleCalls : COUNTER.componentCalls, 1n);
        Atomics.add(counters, COUNTER.perWorkerCallsStart + threadNumber, 1n);
        Atomics.add(counters, COUNTER.inputCodeBytes, BigInt(Buffer.byteLength(code)));
        try {
          const result = compileSource(code, id, corpusDirectory, this);
          Atomics.add(
            counters,
            COUNTER.returnedCodeBytes,
            BigInt(Buffer.byteLength(result.transformResult.code)),
          );
          Atomics.add(counters, COUNTER.returnedMapBytes, BigInt(result.mapBytes));
          Atomics.add(counters, COUNTER.warnings, BigInt(result.warningCount));
          return result.transformResult;
        } catch (error) {
          Atomics.add(counters, COUNTER.errors, 1n);
          throw error;
        } finally {
          const elapsed = process.hrtime.bigint() - startedAt;
          const totalCounter = module ? COUNTER.moduleNsTotal : COUNTER.componentNsTotal;
          const maxCounter = module ? COUNTER.moduleNsMax : COUNTER.componentNsMax;
          Atomics.add(counters, totalCounter, elapsed);
          updateMax(counters, maxCounter, elapsed);
          Atomics.sub(counters, COUNTER.active, 1n);
        }
      },
    },
  };
};

export const svelteRegistryGraphPlugin = (options) => createSvelteRegistryGraphPlugin(options);

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const packageDir = path.dirname(require.resolve('rolldown/package.json'));
const distDir = path.join(packageDir, 'dist');
const bindingPath = path.join(distDir, 'rolldown-binding.wasi.cjs');
const emnapiRuntime = require('@emnapi/runtime');
const createContext = emnapiRuntime.createContext;
const operationTimeout = 30_000;

let firstContext;
let replacementContext;

try {
  const first = loadFreshBinding();
  firstContext = first.context;
  assert.equal(first.binding.__rolldownBindingTarget, 'wasi-threads');

  const { rolldown } = await import(
    `${pathToFileURL(path.join(distDir, 'index.mjs')).href}?wasi-context-lifecycle`
  );
  let markLoadStarted;
  const loadStarted = new Promise((resolve) => {
    markLoadStarted = resolve;
  });
  const bundle = await rolldown({
    input: 'virtual:pending-context-cleanup',
    plugins: [
      {
        name: 'pending-context-cleanup',
        resolveId(source) {
          if (source === 'virtual:pending-context-cleanup') {
            return `\0${source}`;
          }
        },
        load(source) {
          if (source === '\0virtual:pending-context-cleanup') {
            markLoadStarted();
            return new Promise(() => {});
          }
        },
      },
    ],
  });
  const pendingGenerate = bundle.generate();
  await withTimeout(loadStarted, 'the pending plugin load did not start');

  firstContext.destroy();
  await assert.rejects(
    withTimeout(pendingGenerate, 'the pending generate promise did not settle during cleanup'),
    (error) => containsMessage(error, 'Async task was cancelled because its runtime stopped'),
  );
  firstContext.destroy();

  const replacement = loadFreshBinding();
  replacementContext = replacement.context;
  assert.notEqual(replacementContext, firstContext);
  assert.equal(replacement.binding.getRuntimeCapabilities().target, 'wasi-threads');
} finally {
  try {
    firstContext?.destroy();
  } catch {}
  try {
    replacementContext?.destroy();
  } catch {}
  emnapiRuntime.createContext = createContext;
  delete require.cache[bindingPath];
}

console.log('WASI loader context cleanup and reload completed');

function loadFreshBinding() {
  delete require.cache[bindingPath];
  const contexts = [];
  emnapiRuntime.createContext = function captureWasiContext(...args) {
    const context = createContext.apply(this, args);
    contexts.push(context);
    return context;
  };
  let binding;
  try {
    binding = require(bindingPath);
  } finally {
    emnapiRuntime.createContext = createContext;
  }
  assert.equal(contexts.length, 1);
  return { binding, context: contexts[0] };
}

function containsMessage(error, expected) {
  if (String(error?.message ?? error).includes(expected)) {
    return true;
  }
  const nestedErrors =
    typeof error === 'object' && error !== null && Array.isArray(error.errors) ? error.errors : [];
  return nestedErrors.some((entry) => containsMessage(entry, expected));
}

function withTimeout(promise, message) {
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), operationTimeout);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

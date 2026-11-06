import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// https://github.com/rolldown/rolldown/issues/9339
test('hash is stable when an unrelated isolated entry is added', async () => {
  const modules: Record<string, string> = {
    '/node_modules/react/index.js': 'exports.useState = function useState() {};',
    './src/a.js': 'import { useState } from "react"; console.log("a", useState);',
    './src/b.js': 'import { useState } from "react"; console.log("b", useState);',
    './src/c.js': 'import { useState } from "react"; console.log("c", useState);',
  };

  const virtualPlugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
      if (id === 'react') return '/node_modules/react/index.js';
    },
    load(id) {
      return modules[id];
    },
  };

  async function build(input: string[]) {
    const bundle = await rolldown({ input, plugins: [virtualPlugin] });
    const out = await bundle.generate({
      entryFileNames: 'entries-[name]-[hash].js',
      chunkFileNames: 'chunk-[name]-[hash].js',
      codeSplitting: {
        groups: [{ name: 'react', test: /node_modules[\\/]react/ }],
      },
      format: 'esm',
    });
    await bundle.close();
    return new Map(
      out.output.filter((c): c is OutputChunk => c.type === 'chunk').map((c) => [c.name, c]),
    );
  }

  const two = await build(['./src/a.js', './src/b.js']);
  const three = await build(['./src/a.js', './src/b.js', './src/c.js']);

  for (const name of ['rolldown-runtime', 'react', 'a', 'b']) {
    expect(three.get(name)?.code).toBe(two.get(name)?.code);
    expect(three.get(name)?.fileName).toBe(two.get(name)?.fileName);
  }
});

// https://github.com/rolldown/rolldown/issues/5269
// The symbol-name tie-breaker still tied a module namespace with a same-named local.
test('cross-chunk export names are stable across module load orders', async ({
  onTestFinished,
  signal,
}) => {
  const dir = await mkdtemp(path.join(tmpdir(), 'rolldown-export-order-'));
  const paddingDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'padding0');
  let activeWork: Promise<unknown> | undefined;
  onTestFinished(async () => {
    await activeWork?.catch(() => {});
    if (paddingDescriptor) Object.defineProperty(globalThis, 'padding0', paddingDescriptor);
    else Reflect.deleteProperty(globalThis, 'padding0');
    await rm(dir, { recursive: true, force: true });
  });
  const modules = {
    'wrapper.js': 'export * from "./branch-a.js";\nimport "./branch-b.js";\n',
    'branch-a.js': 'export * from "./server.js";\n',
    'branch-b.js': 'import "./padding0.js";\n',
    'padding0.js': 'globalThis.padding0 = true;\n',
    'server.js': [
      'var server_exports = { value: "retained" };',
      'export { server_exports as dt };',
      'export const read = () => import("./consumer.js").then(m => m.read());',
      'export const value0 = 0;',
      '',
    ].join('\n'),
    'consumer.js': [
      'import { dt } from "./server.js";',
      'export const read = () => import("./server.js").then(ns => [ns.dt.value,dt.value]);',
      '',
    ].join('\n'),
    'package.json': '{"type":"module"}',
  };
  activeWork = Promise.all(
    Object.entries(modules).map(([name, code]) => writeFile(path.join(dir, name), code)),
  );
  await activeWork;
  signal.throwIfAborted();

  async function build(first: 'branch-a.js' | 'branch-b.js') {
    signal.throwIfAborted();
    const second = first === 'branch-a.js' ? 'branch-b.js' : 'branch-a.js';
    const firstChild = first === 'branch-a.js' ? 'server.js' : 'padding0.js';
    const secondChild = first === 'branch-a.js' ? 'padding0.js' : 'server.js';
    const pending = new Map<string, () => void>();
    const parsed: string[] = [];
    let releasedSecond = false;
    const bundle = await rolldown({
      cwd: dir,
      input: './wrapper.js',
      plugins: [
        {
          name: 'controlled-parent-load-order',
          async load(id) {
            const name = path.basename(id);
            if (name !== 'branch-a.js' && name !== 'branch-b.js') return null;
            signal.throwIfAborted();
            const code = await readFile(id, 'utf8');
            signal.throwIfAborted();
            return new Promise<string>((resolve, reject) => {
              const onAbort = () => {
                signal.removeEventListener('abort', onAbort);
                reject(signal.reason);
              };
              signal.addEventListener('abort', onAbort, { once: true });
              if (signal.aborted) {
                onAbort();
                return;
              }
              pending.set(name, () => {
                signal.removeEventListener('abort', onAbort);
                resolve(code);
              });
              if (pending.size === 2) pending.get(first)!();
            });
          },
          moduleParsed(info) {
            const name = path.basename(info.id);
            if (name !== 'server.js' && name !== 'padding0.js') return;
            parsed.push(name);
            // The first parent's child is already allocated before the other parent may complete.
            if (name === firstChild) {
              expect(pending.size).toBe(2);
              expect(releasedSecond).toBe(false);
              releasedSecond = true;
              pending.get(second)!();
            }
          },
        },
      ],
    });
    try {
      signal.throwIfAborted();
      const { output } = await bundle.generate({
        format: 'esm',
        minify: true,
        keepNames: true,
        strictExecutionOrder: true,
        sourcemap: true,
        dir: path.join(dir, 'output'),
      });
      signal.throwIfAborted();
      expect(releasedSecond).toBe(true);
      expect(parsed).toEqual([firstChild, secondChild]);
      const files = new Map(
        output.map((file) => {
          const source = file.type === 'chunk' ? file.code : file.source;
          return [
            file.fileName,
            typeof source === 'string' ? Buffer.from(source) : Buffer.from(source),
          ];
        }),
      );
      for (const chunk of output) {
        if (chunk.type !== 'chunk' || !chunk.map) continue;
        expect(chunk.sourcemapFileName).toBe(`${chunk.fileName}.map`);
        expect(files.get(chunk.sourcemapFileName!)?.equals(Buffer.from(chunk.map.toString()))).toBe(
          true,
        );
      }
      const outputDir = path.join(dir, `result-${first}`);
      await mkdir(outputDir);
      await Promise.all(
        [...files].map(([name, code]) => writeFile(path.join(outputDir, name), code)),
      );
      signal.throwIfAborted();
      const api = await import(
        /* @vite-ignore */ pathToFileURL(path.join(outputDir, 'wrapper.js')).href
      );
      signal.throwIfAborted();
      expect(await api.read()).toEqual(['retained', 'retained']);
      expect(api.dt.value).toBe('retained');
      return files;
    } finally {
      await bundle.close();
    }
  }

  const serverFirst = await (activeWork = build('branch-a.js'));
  const paddingFirst = await (activeWork = build('branch-b.js'));
  expect([...paddingFirst.keys()].sort()).toEqual([...serverFirst.keys()].sort());
  for (const [name, source] of serverFirst) {
    expect(paddingFirst.get(name)?.equals(source), `emitted bytes for ${name}`).toBe(true);
  }
});

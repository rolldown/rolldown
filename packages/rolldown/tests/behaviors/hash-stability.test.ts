import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

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
      out.output
        .filter((c): c is OutputChunk => c.type === 'chunk')
        .map((c) => [c.name, c]),
    );
  }

  const two = await build(['./src/a.js', './src/b.js']);
  const three = await build(['./src/a.js', './src/b.js', './src/c.js']);

  for (const name of ['rolldown-runtime', 'react', 'a', 'b']) {
    expect(three.get(name)?.code).toBe(two.get(name)?.code);
    expect(three.get(name)?.fileName).toBe(two.get(name)?.fileName);
  }
});

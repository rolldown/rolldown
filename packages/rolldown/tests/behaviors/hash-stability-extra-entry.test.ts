import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules: Record<string, string> = {
  '/node_modules/react/index.js': 'exports.useState = function useState() {};',
  './src/a.js': 'import { useState } from "react"; console.log("a", useState);',
  './src/b.js': 'import { useState } from "react"; console.log("b", useState);',
  './src/c.js': 'import { useState } from "react"; console.log("c", useState);',
};

const virtualPlugin: Plugin = {
  name: 'virtual',
  resolveId(id) {
    if (id === 'react') {
      return '/node_modules/react/index.js';
    }
    if (id in modules) {
      return id;
    }
  },
  load(id) {
    return modules[id];
  },
};

async function generateOutputByName(input: string[]) {
  const bundle = await rolldown({
    input,
    plugins: [virtualPlugin],
  });
  const output = await bundle.generate({
    entryFileNames: 'entries-[name]-[hash].js',
    chunkFileNames: 'chunk-[name]-[hash].js',
    codeSplitting: {
      groups: [
        {
          name: 'react',
          test: /node_modules[\\/]react/,
        },
      ],
    },
    format: 'esm',
  });
  await bundle.close();

  return new Map(
    output.output
      .filter((chunk): chunk is OutputChunk => chunk.type === 'chunk')
      .map((chunk) => [chunk.name, chunk]),
  );
}

test('rolldown runtime hash is stable when adding an isolated entry', async () => {
  const twoEntryOutput = await generateOutputByName(['./src/a.js', './src/b.js']);
  const threeEntryOutput = await generateOutputByName(['./src/a.js', './src/b.js', './src/c.js']);

  for (const name of ['rolldown-runtime', 'react', 'a', 'b']) {
    expect(threeEntryOutput.get(name)?.code).toBe(twoEntryOutput.get(name)?.code);
    expect(threeEntryOutput.get(name)?.fileName).toBe(twoEntryOutput.get(name)?.fileName);
  }
});

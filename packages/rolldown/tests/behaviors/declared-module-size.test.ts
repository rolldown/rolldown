import type { OutputOptions, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';
import { getOutputChunk } from '../src/utils';

const modules: Record<string, string> = {
  'a.js': `import { shared } from './shared.js'; console.log('a', shared);`,
  'b.js': `import { shared } from './shared.js'; console.log('b', shared);`,
  // About 55 bytes of code.
  'shared.js': `export const shared = { tag: String(globalThis.tag) };`,
};

function virtual(): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      const name = id.replace(/^\.\//, '');
      return name in modules ? name : undefined;
    },
    load(id) {
      return modules[id];
    },
  };
}

function declareSize(size: number): Plugin {
  return {
    name: 'declare-size',
    transform: (code, id) => (id === 'shared.js' ? { code, declaredModuleSize: size } : undefined),
  };
}

async function build(plugins: Plugin[], output: OutputOptions, pluginsFirst = false) {
  const bundle = await rolldown({
    input: ['a.js', 'b.js'],
    preserveEntrySignatures: false,
    plugins: pluginsFirst ? [...plugins, virtual()] : [virtual(), ...plugins],
  });
  try {
    return getOutputChunk(await bundle.generate({ format: 'esm', ...output }));
  } finally {
    await bundle.close();
  }
}

const sharedChunkKept = (chunks: Awaited<ReturnType<typeof build>>) =>
  chunks.some((chunk) => !chunk.isEntry && chunk.moduleIds.includes('shared.js'));

const inline = (maxSize: number): OutputOptions => ({
  codeSplitting: { experimentalInlineCommonChunks: { maxSize } },
});

describe('declaredModuleSize', () => {
  test('replaces the code size in experimentalInlineCommonChunks.maxSize', async () => {
    expect(sharedChunkKept(await build([], inline(1000)))).toBe(false);
    expect(sharedChunkKept(await build([declareSize(1000)], inline(1000)))).toBe(true);
    expect(sharedChunkKept(await build([], inline(20)))).toBe(true);
    expect(sharedChunkKept(await build([declareSize(10)], inline(20)))).toBe(false);
  });

  test('replaces the code size in the size options of codeSplitting groups', async () => {
    const group: OutputOptions = {
      codeSplitting: { groups: [{ name: 'small', test: /shared/, maxModuleSize: 1000 }] },
    };
    const captured = (chunks: Awaited<ReturnType<typeof build>>) =>
      chunks.some((chunk) => chunk.name === 'small');
    expect(captured(await build([], group))).toBe(true);
    expect(captured(await build([declareSize(1001)], group))).toBe(false);
  });

  test('a later hook replaces the value of an earlier one', async () => {
    const loadWithSize: Plugin = {
      name: 'load-with-size',
      load: (id) =>
        id === 'shared.js' ? { code: modules[id], declaredModuleSize: 1000 } : undefined,
    };
    expect(sharedChunkKept(await build([loadWithSize], inline(1000), true))).toBe(true);
    expect(sharedChunkKept(await build([loadWithSize, declareSize(0)], inline(1000), true))).toBe(
      false,
    );
  });

  test.each([-1, 1.5, Number.NaN, Infinity])('rejects %s', async (size) => {
    await expect(build([declareSize(size)], inline(1000))).rejects.toThrow(
      '`declaredModuleSize` must be a non-negative safe integer',
    );
  });
});

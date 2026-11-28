import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';
import { virtualPlugin } from '../src/inline-common-chunks/harness';
import { getOutputChunk } from '../src/utils';

test('uses the UTF-8 source size when inlining native JSON', async () => {
  const modules: Record<string, string> = {
    './a.js': `import data from './shared.json'; console.log('a', data[globalThis.key]);`,
    './b.js': `import data from './shared.json'; console.log('b', data[globalThis.key]);`,
    // 114 UTF-16 code units and 314 UTF-8 bytes straddle maxSize.
    './shared.json': JSON.stringify({ payload: '界'.repeat(100) }),
  };
  const bundle = await rolldown({
    input: ['./a.js', './b.js'],
    preserveEntrySignatures: false,
    plugins: [virtualPlugin(modules)],
  });
  try {
    const chunks = getOutputChunk(
      await bundle.generate({
        format: 'esm',
        codeSplitting: { experimentalInlineCommonChunks: { maxSize: 256 } },
      }),
    );
    const dataChunks = chunks.filter((chunk) => chunk.moduleIds.includes('./shared.json'));
    expect(dataChunks).toHaveLength(1);
    expect(dataChunks[0].isEntry).toBe(false);
  } finally {
    await bundle.close();
  }
});

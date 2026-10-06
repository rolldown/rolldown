import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const source = 'console.log("π 😀");';

async function generate(asciiOnly: boolean) {
  const plugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id === 'entry') return id;
    },
    load(id) {
      if (id === 'entry') return source;
    },
  };
  const bundle = await rolldown({ input: 'entry', plugins: [plugin] });
  try {
    const output = await bundle.generate({
      minify: {
        compress: false,
        mangle: false,
        codegen: { removeWhitespace: false, asciiOnly },
      },
    });
    return output.output.find((item): item is OutputChunk => item.type === 'chunk')!.code;
  } finally {
    await bundle.close();
  }
}

test('output minification propagates codegen.asciiOnly', async () => {
  const ascii = await generate(true);
  expect(ascii).not.toContain('π');
  expect(ascii).not.toContain('😀');
  expect(ascii).toContain('\\u');

  const unicode = await generate(false);
  expect(unicode).toContain('π 😀');
});

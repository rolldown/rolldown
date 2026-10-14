import type { OutputChunk, OutputOptions, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const source = 'export const π = "你好 😀"; console.log(π);';

function isAscii(value: string) {
  for (let index = 0; index < value.length; index++) {
    if (value.charCodeAt(index) > 0x7f) return false;
  }
  return true;
}

async function generate(outputOptions: OutputOptions = {}, addUnicodeInRenderChunk = false) {
  const plugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id === 'entry') return id;
    },
    load(id) {
      if (id === 'entry') return source;
    },
    renderChunk(code) {
      if (addUnicodeInRenderChunk) return `${code}\nconsole.log("追加");`;
    },
  };
  const bundle = await rolldown({ input: 'entry', plugins: [plugin] });
  try {
    const output = await bundle.generate(outputOptions);
    return output.output.find((item): item is OutputChunk => item.type === 'chunk')!.code;
  } finally {
    await bundle.close();
  }
}

test('output.charset defaults to utf8', async () => {
  const output = await generate();
  expect(output).toContain('π');
  expect(output).toContain('你好 😀');
});

test('output.charset: utf8 preserves non-ASCII characters', async () => {
  const output = await generate({ charset: 'utf8' });
  expect(output).toContain('π');
  expect(output).toContain('你好 😀');
});

test('output.charset: ascii escapes non-ASCII characters', async () => {
  const output = await generate({ charset: 'ascii' });
  expect(isAscii(output)).toBe(true);
  expect(output).toContain('\\u03C0');
  expect(output).toContain('\\u4F60\\u597D \\u{1F600}');
});

test('output.charset: ascii applies when minifying', async () => {
  const output = await generate(
    {
      charset: 'ascii',
      minify: {
        compress: false,
        mangle: false,
        codegen: { removeWhitespace: false, asciiOnly: false },
      },
    },
    true,
  );
  expect(isAscii(output)).toBe(true);
  expect(output).toContain('\\u8FFD\\u52A0');
});

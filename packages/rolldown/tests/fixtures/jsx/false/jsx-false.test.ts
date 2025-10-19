import { expect, test } from 'vitest'
import { rolldown } from 'rolldown'

test('jsx false should report error', async () => {
  try {
    const build = await rolldown({
      input: './main.jsx',
      cwd: import.meta.dirname,
      moduleTypes: {
        '.jsx': 'js',
        '.tsx': 'ts',
      },
    })
    await build.write({})
  } catch (e: any) {
    expect(e.message).toContain('PARSE_ERROR')
  }
})

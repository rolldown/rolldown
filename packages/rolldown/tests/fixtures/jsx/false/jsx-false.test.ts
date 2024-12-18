import { expect, test } from 'vitest'
import { rolldown } from 'rolldown'

test('jsx false should report error', async () => {
  try {
    const build = await rolldown({
      input: './main.jsx',
      cwd: import.meta.dirname,
      jsx: false,
    })
    await build.write({})
  } catch (e: any) {
    expect(e.message).toContain('PARSE_ERROR')
  }
})

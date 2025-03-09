import { parseAst, parseAstAsync } from 'rolldown/parseAst'
import { test, expect } from 'vitest'

test('rolldown/parseAst parseSync', async () => {
  const result = parseAst('console.log("hello")')
  expect(result.type).toBe('Program')
})

test('rolldown/parseAst parseAstAsync', async () => {
  const result = await parseAstAsync('console.log("hello")')
  expect(result.type).toBe('Program')
})

test('rolldown/parseAst parseSync + error', async () => {
  try {
    parseAst('\nconso le.log("hello")')
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`"Failed to get constructor of class \`ParseResult\` in \`ToNapiValue\`"`)
  }
})

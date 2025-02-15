import { parseAst, parseAstAsync } from 'rolldown/parseAst'
import { test, expect } from 'vitest'

test('rolldown/parseAst parseSync', async () => {
  const result = parseAst('test.js', 'console.log("hello")')
  expect(result.type).toBe('Program')
})

test('rolldown/parseAst parseAstAsync', async () => {
  const result = await parseAstAsync('test.js', 'console.log("hello")')
  expect(result.type).toBe('Program')
})

test('rolldown/parseAst parseSync + error', async () => {
  try {
    parseAst('test.js', '\nconso le.log("hello")')
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`
      "Parse failed with 1 error:
      Expected a semicolon or an implicit semicolon after a statement, but found none
      1: 
      2: conso le.log("hello")
              ^"
    `)
  }
})

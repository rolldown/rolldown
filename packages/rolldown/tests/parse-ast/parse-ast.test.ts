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

test('rolldown/parseAst non json value', async () => {
  const result = await parseAstAsync('1n')
  expect(result.body[0]).toMatchInlineSnapshot(`
    {
      "end": 2,
      "expression": {
        "bigint": "1",
        "end": 2,
        "raw": "1n",
        "start": 0,
        "type": "Literal",
        "value": 1n,
      },
      "start": 0,
      "type": "ExpressionStatement",
    }
  `)
})

test('rolldown/parseAst parseSync + error', async () => {
  try {
    parseAst('\nconso le.log("hello")')
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

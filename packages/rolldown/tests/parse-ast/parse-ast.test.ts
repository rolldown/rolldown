import { parseAst, parseAstAsync } from 'rolldown/parseAst'
import { test, expect } from 'vitest'

test('rolldown/parseAst parseSync', async () => {
  const result = parseAst('test.js', 'console.log("hello")')
  expect(result.program.type).toBe('Program')
})

test('rolldown/parseAst parseAstAsync', async () => {
  const result = await parseAstAsync('test.js', 'console.log("hello")')
  expect(result.program.type).toBe('Program')
})

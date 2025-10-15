import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: 'preserve'
    }
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.replace(/\s+/g, '')).toBe(`//#regionmain.jsxconsole.log(<div>test</div>);//#endregion`)
  },
})

import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import {OutputChunk} from 'rolldown'
import { transformPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    output: {
      preserveModules: true,
      format: 'cjs',
    },
    plugins: []
  },
  afterTest: (output) => {
    console.log('Generated files:');
    output.output.forEach((chunk, index) => {
      console.log(`--- File ${index}: ${chunk.fileName} ---`);
      console.log(chunk.code || chunk.source);
    });
    
    expect(output.output[0].fileName).toMatchInlineSnapshot(`"packages/rolldown/tests/fixtures/topics/preserve-modules/objectspread-cjs/main.js"`);
    const mainChunk = output.output[0] as OutputChunk;
    
    // Look for the specific bug pattern described in the issue
    // The bug WAS: require_objectSpread2$1.require_objectSpread2() instead of require_objectSpread2$1
    // After the fix, this should no longer occur
    expect(mainChunk.code).not.toMatch(/require_objectSpread2\$\d+\.require_objectSpread2\(\)/);
    
    // The correct code should use the require variable directly in __toESM
    expect(mainChunk.code).toMatch(/__toESM\(require_objectSpread2\$\d+, 1\)/);
  },
})
// Test if `freeExternalMemory` could be called satisfying the type requirements

import { rolldown } from 'rolldown'
import { freeExternalMemory } from 'rolldown/experimental'

async function testFreeExternalMemory() {
  const build = await rolldown({
    input: './main.ts',
  })

  const bundle = await build.generate()
  

  function _usage() {
    freeExternalMemory(bundle)
    for (const item of bundle.output) {
      freeExternalMemory(item)
    }
  }
}

export default testFreeExternalMemory

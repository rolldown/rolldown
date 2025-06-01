import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  config: {
    input: entry,
    external: /\.\/bar.js/,
    plugins: [{
      name: 'test',
      resolveId(source) {
        if (source === './foo') {
          return './bar.js';
        }
      },
      buildEnd() {
        const target = [...this.getModuleIds()].find((v) => v === './bar.js');
        expect(target).toBeDefined()
      },
    }],
  }
})

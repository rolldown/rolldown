import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      advancedChunks: {
        groups: [
          {
            name(id) {
              if (/node_modules[\\/]+lib-ui/.test(id)) {
                return 'ui';
              }
              if (/[\\/]node_modules/.test(id)) {
                return 'other-libs';
              }

              return null;
            }
          },
          
        ],
      },
    },
  },
  afterTest(output) {
    function findChunkStartWith(prefix: string) {
      const finded = output.output.find(chunk => chunk.type === 'chunk' && chunk.fileName.startsWith(prefix));
      if (!finded) {
        throw new Error(`chunk ${prefix} not found`)
      }
      if (finded.type !== 'chunk') {
        throw new Error('should be chunk')
      }
      return finded;
    }
    const ui = findChunkStartWith('ui-')
    const otherLibs = findChunkStartWith('other-libs-')

    expect(ui.moduleIds).toMatchObject([
      /lib-ui[\\/]index.js$/,
    ])

    expect(otherLibs.moduleIds).toMatchObject([
      /lib-npm-a[\\/]index.js$/,
      /lib-npm-b[\\/]index.js$/,
    ])
  },
})

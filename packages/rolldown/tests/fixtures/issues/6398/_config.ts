import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    external: ['node:assert'],
    plugins: [{
      name: 'test',
      async resolveId(specifier, importer, extraArgs) {
        if (specifier === 'dep') {
          return await this.resolve(specifier, importer)
        }
      }
    }]
  },
  async afterTest() {
     // @ts-ignore
     await import('./dist/main')
  },
})

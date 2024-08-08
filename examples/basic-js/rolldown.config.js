import { defineConfig } from 'rolldown'

export default defineConfig({
  input: {
    entry: './index.js',
  },
  plugins: [
    {
      name() {
        return 'test'
      },
      resolveId(id) {
        if (id === 'test.js') {
          return id
        }
      },
      load() {
        return {
          code: 'export default {}',
          map: undefined,
        }
      },
    },
  ],
})

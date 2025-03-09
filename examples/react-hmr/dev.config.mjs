import { defineDevConfig } from '@rolldown/test-dev-server'

export default defineDevConfig({
  build: {
    input: 'src/main.tsx',
    experimental: {
      hmr: true,
    },
    treeshake: false,
  },
})

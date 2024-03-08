import type { RollupOptions } from '@rolldown/node'

const config: RollupOptions = {
  resolve: {
    alias: {
      '@': __dirname,
    },
  },
}

export default {
  config,
}

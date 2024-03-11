import type { RollupOptions } from 'rolldown'

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

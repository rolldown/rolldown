import type { RollupOptions } from '../../../../src'

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

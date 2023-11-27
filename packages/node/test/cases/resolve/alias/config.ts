import type { RollupOptions } from '@rolldown/node'
import path from 'path'

const config: RollupOptions = {
  resolve: {
    alias: {
      '@': path.dirname(__dirname),
    },
  },
}

export default {
  config,
}

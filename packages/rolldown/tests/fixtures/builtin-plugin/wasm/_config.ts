import { wasmFallbackPlugin, wasmPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [wasmPlugin(), wasmFallbackPlugin()],
  },
})

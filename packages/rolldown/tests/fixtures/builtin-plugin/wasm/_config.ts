import { wasmFallbackPlugin, wasmHelperPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [wasmHelperPlugin(), wasmFallbackPlugin()],
  },
})

import { wasmFallbackPlugin, wasmHelperPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [wasmHelperPlugin(), wasmFallbackPlugin()],
  },
})

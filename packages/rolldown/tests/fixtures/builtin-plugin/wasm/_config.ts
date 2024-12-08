import { wasmFallbackPlugin, wasmHelperPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  skip: true, // FIXME(hyf0): this test is not working already.
  config: {
    plugins: [wasmHelperPlugin(), wasmFallbackPlugin()],
  },
})

import { wasmFallbackPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [wasmFallbackPlugin()],
  },
  catchError: () => {
    // Errors are swallowed here
  },
})

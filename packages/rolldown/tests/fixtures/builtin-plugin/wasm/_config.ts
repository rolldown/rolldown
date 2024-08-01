import { wasmPlugin } from '../../../../'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [wasmPlugin()],
  },
})

import { BuiltinWasmPlugin } from '../../../../'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [new BuiltinWasmPlugin()],
  },
})

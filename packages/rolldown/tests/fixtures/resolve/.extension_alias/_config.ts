import { defineTest } from '@tests'

export default defineTest({
  config: {
    resolve: {
      extensionAlias: { '.ts': ['.ts', '.js'] },
    },
  },
})

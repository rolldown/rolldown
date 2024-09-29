import { defineTest } from '@tests'
import path from 'node:path'
const entry = path.join(__dirname, './main.ts')

export default defineTest({
  config: {
    input: entry,
    resolve: {
      extensionAlias: { '.ts': ['.ts', '.js'] },
    },
  },
})

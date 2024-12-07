import { defineTest } from '@tests'
import path from 'node:path'
const entry = path.join(__dirname, './main.ts')

export default defineTest({
  skip: true, // FIXME(hyf0): this test is not working already.
  config: {
    input: entry,
    resolve: {
      extensionAlias: { '.ts': ['.ts', '.js'] },
    },
  },
})

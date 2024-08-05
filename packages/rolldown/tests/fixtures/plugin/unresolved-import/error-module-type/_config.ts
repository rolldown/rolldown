import { defineTest } from '@tests'
import path from 'node:path'

const entry = path.join(__dirname, './main.javascript')

export default defineTest({
  skip: true,
  skipComposingJsPlugin: true,
  config: {
    input: entry,
    plugins: [],
  },
})

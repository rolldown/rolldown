import { defineParallelPlugin } from 'rolldown/experimental'
import path from 'node:path'

export default defineParallelPlugin(
  path.resolve(import.meta.dirname, './impl.js'),
)

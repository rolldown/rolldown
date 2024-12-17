import { defineParallelPlugin } from 'rolldown/experimental'
import path from 'node:path'

/** @type {import('rolldown').DefineParallelPluginResult<void>} */
export default defineParallelPlugin(
  path.resolve(import.meta.dirname, './impl.js'),
)

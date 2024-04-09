import { defineThreadSafePlugin } from 'rolldown'
import path from 'node:path'

/** @type {import('rolldown').DefineThreadSafePluginResult<void>} */
export default defineThreadSafePlugin(
  path.resolve(import.meta.dirname, './impl.js'),
)

import path from 'node:path';
import { defineParallelPlugin } from 'rolldown/experimental';

/** @type {import('rolldown').DefineParallelPluginResult<{ mode: 'init' | 'sync' | 'reject' }>} */
export default defineParallelPlugin(path.resolve(import.meta.dirname, './impl.js'));

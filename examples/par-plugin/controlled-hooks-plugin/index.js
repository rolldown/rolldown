import nodePath from 'node:path';
import { defineParallelPlugin } from 'rolldown/experimental';

export default defineParallelPlugin(nodePath.resolve(import.meta.dirname, './impl.js'));

import { a } from './x.js';

(globalThis.entryLog ??= []).push(`y:${a}`);

import { a } from './lib.js';

(globalThis.__events ??= []).push('entry ' + a);

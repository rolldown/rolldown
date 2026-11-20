import { SIZE, name } from './ui/index.js';

(globalThis.loadOrder ??= []).push(`main ${SIZE} ${name()}`);

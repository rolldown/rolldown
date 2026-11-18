import { lazyValueA1, lazyValueA2 } from './library/index.js';

(globalThis.fixtureLog ??= []).push(`lazy-a:${lazyValueA1()}+${lazyValueA2()}`);

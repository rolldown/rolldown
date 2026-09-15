import { lazyValueC } from './library/index.js';

(globalThis.fixtureLog ??= []).push(`lazy-c:${lazyValueC()}`);

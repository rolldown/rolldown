import { lazyValueB } from './library/index.js';

(globalThis.fixtureLog ??= []).push(`lazy-b:${lazyValueB()}`);

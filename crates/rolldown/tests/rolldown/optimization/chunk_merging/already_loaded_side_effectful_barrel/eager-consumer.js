import { eagerValueB } from './library/index.js';

(globalThis.fixtureLog ??= []).push(`eager:${eagerValueB()}`);

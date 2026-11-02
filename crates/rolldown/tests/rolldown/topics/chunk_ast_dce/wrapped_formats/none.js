import { read } from './private.js';

globalThis.__wrappedEvents.push('no-exports-wrapper-anchor');
globalThis.__wrappedResult = read();

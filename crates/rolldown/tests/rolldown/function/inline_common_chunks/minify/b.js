import { bump, count } from './shared.js';
globalThis.events.push('B ' + count + ' ' + bump());

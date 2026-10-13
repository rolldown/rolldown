import { bump, count } from './shared.js';
globalThis.events.push('C ' + count + ' ' + bump());

import { bump, count } from './shared.js';
globalThis.events.push('A ' + count + ' ' + bump());

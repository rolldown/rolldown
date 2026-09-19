import * as state from './barrel.js';

globalThis.events.push(`same:${state === globalThis.firstStateNamespace}`);
globalThis.events.push(`live:${state.count}`);
globalThis.events.push(state.bump());

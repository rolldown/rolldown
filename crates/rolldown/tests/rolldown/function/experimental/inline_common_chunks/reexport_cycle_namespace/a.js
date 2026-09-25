import * as state from './barrel.js';

globalThis.firstStateNamespace = state;
globalThis.events.push(state.bump());

import { increment, value } from './shared.js';

increment();
globalThis.events.push(`a:${value}`);

import { save } from './action.js';
import { remove } from './action-without-earlier-map.js';
globalThis.actions = { save, remove };

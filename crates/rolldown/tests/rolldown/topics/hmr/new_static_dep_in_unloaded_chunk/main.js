import './hmr.js';
import { load } from './lazy-holder.js';

// Never call `load`: heavy.js stays an unloaded chunk in this client.
console.log(typeof load);

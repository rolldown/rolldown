import './hmr.js';
import { shared } from './shared.js';
import { load } from './holder.js';

// Never call `load`: route.js stays an unloaded chunk in this client.
console.log(shared, typeof load);

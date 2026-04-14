import { value } from './deep.js';

const manager = { value, ready: false };

function setup() {
  manager.ready = true;
}

export { manager, setup };

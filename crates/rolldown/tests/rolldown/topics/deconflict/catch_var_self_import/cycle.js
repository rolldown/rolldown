import { cycle as alias } from './barrel.js';

export var cycle = 10;
cycle++;
export let result;
try {
  throw 0;
} catch (cycle) {
  var cycle = 20;
  result = alias;
}

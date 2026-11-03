import { value } from './tla-export.js';

let result;
try {
  result = `value = ${value}`;
} catch (e) {
  result = `${e.name}: ${e.message}`;
}
document.getElementById('top-level-await-tdz-status').textContent = result;

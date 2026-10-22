import { x } from './b.js';
function ignore(_value) {
  return 'called';
}
let result;
try {
  result = ignore(x);
} catch {
  result = 'tdz';
}
export { result };

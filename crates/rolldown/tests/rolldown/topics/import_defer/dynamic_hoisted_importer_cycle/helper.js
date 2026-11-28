import './trigger.js';
import defer * as ns from './target.js';
export function load() {
  return import.defer('./target.js');
}

import * as shadowed from './shadowed.js';
import * as ambiguous from './ambiguous.js';
import * as defaulted from './defaulted.js';

export const entries = {
  shadowed: Object.entries(shadowed),
  ambiguous: Object.entries(ambiguous),
  defaulted: Object.entries(defaulted),
};
export async function read() {
  return Object.entries(await import('./lazy.js'));
}

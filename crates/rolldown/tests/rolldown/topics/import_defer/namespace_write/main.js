import { log } from './log.js';
import defer * as ns from './x.js';

const errorName = (fn) => {
  try {
    fn();
    return 'none';
  } catch (error) {
    return error.constructor.name;
  }
};

export const setError = errorName(() => {
  ns.value = 2;
});
export const afterSet = [...log];
export const updateError = errorName(() => {
  ns.value += 1;
});
export const afterUpdate = [...log];
export const deleteError = errorName(() => {
  delete ns.value;
});
export const value = ns.value;

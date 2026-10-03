export const DEV = false;
export const PROD = true;
export let mutable = false;
mutable = globalThis.mutable;
export const object = {
  toString() {
    console.log('COERCION');
    return '';
  },
};

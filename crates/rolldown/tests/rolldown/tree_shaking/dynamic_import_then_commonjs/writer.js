import target from './patched.cjs';

export function patch() {
  target.then = function (resolve) {
    globalThis.thenCalled('patched');
    resolve({});
  };
}

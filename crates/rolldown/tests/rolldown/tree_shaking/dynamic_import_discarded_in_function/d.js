export const d1 = 'd1';
// Promise resolution calls `then` with the namespace as `this`.
export function then(resolve) {
  globalThis.resolveD(this.d1);
  resolve({});
}

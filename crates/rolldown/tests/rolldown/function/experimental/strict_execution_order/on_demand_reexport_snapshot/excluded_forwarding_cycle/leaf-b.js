function value() {
  globalThis.orderValue = 'after';
  return 'b';
}
export const BValue = value();

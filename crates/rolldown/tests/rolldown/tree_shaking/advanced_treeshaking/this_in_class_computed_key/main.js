function make() {
  class A {
    [this.name]() {
      return 'method';
    }
  }
  return A;
}
const o = { name: 'run', make };
export const C = o.make();

export let value = 'expected';
export function read() {
  return (() => this.value)();
}

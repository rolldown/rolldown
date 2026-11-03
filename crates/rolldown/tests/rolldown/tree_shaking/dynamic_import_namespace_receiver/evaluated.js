export let value = 'expected';
export function read() {
  return eval('this.value');
}

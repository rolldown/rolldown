export let value = 'expected';
export function then(resolve) {
  resolve({ value: 'assimilated:' + this.value });
}

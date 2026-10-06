export let value = 1;

export function setValue(next) {
  value = next;
}

export function unrelated() {
  return 'unrelated';
}

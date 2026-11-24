console.log('leaf');
export function then() {
  throw new Error('then called');
}

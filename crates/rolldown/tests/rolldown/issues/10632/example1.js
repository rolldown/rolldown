const value = await import('./example2');
const again = await import('./example2');

if (!value.example2 || !again.example2 || value !== again || globalThis.example2Runs !== 1) {
  throw new Error('Expected one target evaluation and one namespace');
}

export default value;

export async function example1() {
  console.log('example1');
}

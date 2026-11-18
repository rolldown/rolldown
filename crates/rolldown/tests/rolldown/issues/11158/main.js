import { lib } from './lib.js';

export default async function main() {
  const { load } = await import('./adapter.js');
  return lib() + (await load());
}

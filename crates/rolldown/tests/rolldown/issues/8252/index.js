import { createSorter } from './sorter';

export async function main() {
  const v3 = await import('./v3');
  return { sorter: createSorter(), v3 };
}

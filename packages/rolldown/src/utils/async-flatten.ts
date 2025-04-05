// Copied from https://github.com/rollup/rollup/blob/3b560f7c889a63968dabc9b6970aabf52a77d3fd/src/utils/asyncFlatten.ts

export async function asyncFlatten<T>(array: T[]): Promise<T[]> {
  do {
    array = (await Promise.all(array)).flat(Infinity) as any;
  } while (array.some((v: any) => v?.then));
  return array;
}

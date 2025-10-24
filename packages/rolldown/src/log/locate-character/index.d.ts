export function getLocator(
  source: string,
  options?: Options,
): (
  search: string | number,
  index?: number,
) => Location | undefined;

export function locate(
  source: string,
  search: string | number,
  options?: Options,
): Location | undefined;
export type Location = Location_1;
interface Options {
  offsetLine?: number;
  offsetColumn?: number;
  startIndex?: number;
}

interface Location_1 {
  line: number;
  column: number;
  character: number;
}

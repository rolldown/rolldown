export const {
  x: [a = 1],
} = { x: [] };
export const {
  x: [, b = 2],
} = { x: [] };
export const [
  {
    x: [c = 3],
  },
] = [{ x: [] }];

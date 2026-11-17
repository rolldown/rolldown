// With `propertyWriteSideEffects: false`, tree shaking keeps `api.enable = ...` because `run`
// uses `api`.
export let flag = false;
const api = {};
api.enable = () => {
  flag = true;
};

export function run() {
  api.enable();
}

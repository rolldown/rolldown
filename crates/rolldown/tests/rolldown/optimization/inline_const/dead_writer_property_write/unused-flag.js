// Nothing uses `unused_api`, so tree shaking drops the writer and `unused` is a constant.
export let unused = false;
const unused_api = {};
unused_api.enable = () => {
  unused = true;
};

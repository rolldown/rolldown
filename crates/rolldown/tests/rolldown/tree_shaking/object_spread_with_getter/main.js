// Spreading an inline object with a getter SHOULD be preserved — it has side effects.
let result = 'FAIL';
const unused = {
  ...{
    get prop() {
      result = 'PASS';
    },
  },
};

export { result };

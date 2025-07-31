import objectSpread2 from '@oxc-project/runtime/helpers/objectSpread2';

const foo = {a:1, b:2, c:3};
console.log(objectSpread2({}, foo, { d: 4 }));
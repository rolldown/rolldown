import { load } from './helper.js';
console.log('trigger');
load().then((ns) => {
  console.log('loaded', typeof ns);
  console.log('value', ns.x);
});

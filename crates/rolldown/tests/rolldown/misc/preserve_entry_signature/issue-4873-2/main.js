function dy(path) {
  switch (path) {
    case './foo.js':
      return import('./foo.js');
    case './bar.js':
      return import('./bar.js');
  }
}

let view = 'foo';
const { msg } = await dy(`./${view}.js`);
console.log(msg);

import(`https://localhost`).catch((mod) => {
  console.log(mod);
});

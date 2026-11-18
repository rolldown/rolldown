import './dep.js';

const name = 'dep';
const rest = ['./dep.js'];
const cb = () => {};

import.meta.hot.accept(`./${name}.js`, cb);
import.meta.hot.accept(['./dep.js', name], cb);
import.meta.hot.accept(['./dep.js', `./${name}.js`], cb);
import.meta.hot.accept(['./dep.js', ...rest], cb);
import.meta.hot.accept([, './dep.js'], cb);

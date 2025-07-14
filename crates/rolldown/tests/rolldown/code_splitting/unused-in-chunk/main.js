const templates = {
  foo: () => import('./foo.js'), // two separate dynamic imports (causing separate chunks)
  bar: () => import('./bar.js'), 
};

console.log(templates);

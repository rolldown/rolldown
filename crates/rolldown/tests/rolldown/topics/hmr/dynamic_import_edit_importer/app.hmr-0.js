import.meta.hot.accept();

import('./foo.js').then((mod) => {
  console.log('.app-edited', mod.value);
});

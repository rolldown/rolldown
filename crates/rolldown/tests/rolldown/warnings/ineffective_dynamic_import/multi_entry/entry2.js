// Entry point 2: dynamically imports shared module
// This should trigger a warning because entry1 statically imports it,
// causing shared.js to be in entry1's chunk
import('./shared.js').then(mod => {
  console.log('Entry 2 dynamic:', mod.data);
});

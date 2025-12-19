// This should NOT trigger a warning
// because lazy.js is ONLY dynamically imported (no static import)
import('./lazy.js').then(mod => {
  console.log('Lazy loaded:', mod.data);
});
console.log('Main loaded');

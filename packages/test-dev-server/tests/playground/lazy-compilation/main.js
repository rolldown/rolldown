document.querySelector('.status').textContent = 'main loaded';

// Dynamic import - will be lazy compiled
document.querySelector('.lazy-result').textContent = 'waiting';

setTimeout(async () => {
  const lazyModule = await import('./lazy-module.js');
  document.querySelector('.lazy-result').textContent = lazyModule.value;
}, 1000);

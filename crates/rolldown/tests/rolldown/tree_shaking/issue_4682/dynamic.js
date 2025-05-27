import './dynamic.no-treeshake.js';

export const lazyLoad = async () => {
  await import('./static.js');
  document.body.classList.add('loaded');
};

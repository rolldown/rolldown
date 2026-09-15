import { value } from './child.js';

if (import.meta.hot) {
  import.meta.hot.accept(() => {
    import.meta.hot.invalidate();
  });
}

setTimeout(() => {
  document.querySelector('.invalidation-circular-deps').textContent = value;
});

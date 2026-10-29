import { value } from './child.js';

if (import.meta.hot) {
  import.meta.hot.accept(() => {});
}

setTimeout(() => {
  document.querySelector('.invalidation-circular-deps-handled').textContent = value;
});

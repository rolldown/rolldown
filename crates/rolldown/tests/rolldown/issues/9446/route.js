export default {
  loadString: () => import('./client-only.js').then((r) => r.default || r),
};

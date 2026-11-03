// Re-exports the nested module; `app` accepts THIS module, so editing `nested` must bubble
// nested -> dep -> app to reach the accept-dep boundary.
export { value } from './nested.js';

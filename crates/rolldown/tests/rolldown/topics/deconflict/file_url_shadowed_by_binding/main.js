// `import.meta.ROLLDOWN_FILE_URL_*` is printed as `new URL("./asset.txt", import.meta.url).href`.
// Neither the parameter `URL` nor the top-level `URL` of `other.js` may capture that `URL`.
import { other } from './other.js';
export function href(URL) {
  return import.meta.ROLLDOWN_FILE_URL___REF__;
}
export { other };

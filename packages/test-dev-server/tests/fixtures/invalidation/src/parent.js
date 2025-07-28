import { value } from './child';

if (import.meta.hot) {
  import.meta.hot.accept();
}

if (value === 'child-edited') {
  globalThis.nodeFs.writeFileSync('./ok-1', '');
}

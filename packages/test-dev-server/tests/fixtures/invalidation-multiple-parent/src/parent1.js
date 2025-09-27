import assert from 'node:assert';
import { value } from './child';

assert(['child', 'child-updated'].includes(value));

if (import.meta.hot) {
  import.meta.hot.accept();
}

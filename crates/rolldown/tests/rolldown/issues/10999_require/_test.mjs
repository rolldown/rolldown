import { result } from './dist/main.js';

const values = await result;
if (values.join(',') !== 'app-h1,-json,app-json') {
  throw new Error('unexpected result: ' + values.join(','));
}

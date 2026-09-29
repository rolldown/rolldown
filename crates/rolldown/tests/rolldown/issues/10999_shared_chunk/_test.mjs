import { result as result1 } from './dist/main1.js';
import { result as result2 } from './dist/main2.js';

for (const result of [result1, result2]) {
  const values = await result;
  if (values.join(',') !== 'app-h1,-json,app-json') {
    throw new Error('unexpected result: ' + values.join(','));
  }
}

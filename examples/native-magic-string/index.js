import { API_URL } from './config.js';
import { greet } from './greet.js';

console.log(greet('World'));
console.log('API URL:', API_URL);

export { API_URL, greet };

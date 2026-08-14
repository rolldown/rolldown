import { flag } from './flags.js';

if (flag) {
  console.log('should not see me in bundle!');
} else {
  console.log('flag is false correctly');
}

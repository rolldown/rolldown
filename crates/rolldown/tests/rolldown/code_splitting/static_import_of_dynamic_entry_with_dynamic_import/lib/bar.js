import { heavy } from './heavy.js';
import { foo } from './foo.js';

export class Bar {
  value() {
    return heavy() + foo.value;
  }
}

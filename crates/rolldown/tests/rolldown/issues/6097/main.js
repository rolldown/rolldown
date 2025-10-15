import assert from 'node:assert'
import json from './a.json';
class Message {
  id;
  constructor(id) {
    this.id = id;
  }

  toString() {
    return this.id.toString();
  }
}

const y = new Message(1);

assert(y.toString() === '1');
assert(json.Message + y.toString() === '11');

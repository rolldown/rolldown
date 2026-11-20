import assert from 'node:assert';

const reads = [];
main();
callsNested();
const arrow = () => reads.push(['arrow', arrowFlag]);
arrow();
[1].forEach(() => reads.push(['callback', callbackFlag]));
try {
  throw 0;
} catch {
  reads.push(['catch', catchFlag]);
}
class Reader {
  static {
    reads.push(['static', staticFlag]);
  }
}

export var flag = true;
export var nestedFlag = true;
export var arrowFlag = true;
export var callbackFlag = true;
export var catchFlag = true;
export var staticFlag = true;
export var lateFlag = true;
export var initFlag = (readInitFlag(), true);

main();
reads.push(['late', late()]);

function main() {
  reads.push(['main', flag]);
}
function callsNested() {
  nested();
}
function nested() {
  reads.push(['nested', nestedFlag]);
}
function late() {
  return lateFlag;
}
function readInitFlag() {
  reads.push(['initializer', initFlag]);
}

assert.deepStrictEqual(reads, [
  ['main', undefined],
  ['nested', undefined],
  ['arrow', undefined],
  ['callback', undefined],
  ['catch', undefined],
  ['static', undefined],
  ['initializer', undefined],
  ['main', true],
  ['late', true],
]);

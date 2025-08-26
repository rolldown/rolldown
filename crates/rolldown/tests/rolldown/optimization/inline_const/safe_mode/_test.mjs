import fs from 'node:fs';
import assert from 'node:assert';
import path from 'node:path'
// read dist/main.js into string 
const content = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf-8');

if (globalThis.__configName === 'smart-inline-const') {
  assert(!content.includes('unused two'));
  assert(!content.includes('unused three'));
  assert(!content.includes('Production mode code'));
  assert.equal(searchAppearTimes('one === 1'), 0);
  // If the variable did not appear in test expr of conditional statement, it should not be inlined.
  assert(content.includes('console.log(mode, one)'));
} else {
  assert(content.includes('unused two'));
  assert(content.includes('unused three'));
  assert(content.includes('Production mode code'));
  assert.equal(searchAppearTimes('one === 1'), 0);

  assert(content.includes('console.log(mode, one)'));
}


// a function that search needle appeared times.
function searchAppearTimes(haystack, needle) {
  let count = 0;
  let pos = haystack.indexOf(needle);
  while (pos !== -1) {
    count++;
    pos = haystack.indexOf(needle, pos + needle.length);
  }
  return count;
}

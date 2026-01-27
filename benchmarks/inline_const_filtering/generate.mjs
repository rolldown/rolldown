import fs from 'fs';
import path from 'path';

const UNRELATED_MODULE_COUNT = 10000;
const CONSTANT_CHAIN_DEPTH = 15;

const outputDir = path.join(import.meta.dirname, 'src');

// Clean and create output directory
if (fs.existsSync(outputDir)) {
  fs.rmSync(outputDir, { recursive: true });
}
fs.mkdirSync(outputDir, { recursive: true });

// Generate constant chain modules (these require multiple passes)
// const_0.js exports CONST_0 = true
// const_1.js imports CONST_0, exports CONST_1 = CONST_0
// const_2.js imports CONST_1, exports CONST_2 = CONST_1
// etc.
for (let i = 0; i < CONSTANT_CHAIN_DEPTH; i++) {
  let content;
  if (i === 0) {
    content = `export const CONST_${i} = 'value' === 'value';\n`;
  } else {
    content = `import { CONST_${i - 1} } from './const_${i - 1}.js';\nexport const CONST_${i} = CONST_${i - 1};\n`;
  }
  fs.writeFileSync(path.join(outputDir, `const_${i}.js`), content);
}

// Generate many unrelated modules (these should be skipped in pass 2+)
// Each unrelated module exports multiple constants to make AST traversal more expensive
for (let i = 0; i < UNRELATED_MODULE_COUNT; i++) {
  // Generate multiple exported constants per module to increase traversal work
  let content = `// Unrelated module ${i} - should be skipped in subsequent passes\n`;

  // Add 10 exported constants per module (these need to be visited during optimization)
  for (let j = 0; j < 10; j++) {
    content += `export const VALUE_${i}_${j} = ${i * 10 + j};\n`;
  }

  // Add some computed exports that require evaluation
  content += `export const COMPUTED_${i} = ${i} + ${i + 1};\n`;
  content += `export const STRING_${i} = 'module' + '_' + '${i}';\n`;
  content += `export const BOOL_${i} = ${i % 2 === 0};\n`;

  content += `
export function compute_${i}(x) {
  let result = x;
  for (let j = 0; j < 10; j++) {
    result = result * 2 + j;
  }
  return result;
}

export const data_${i} = {
  id: ${i},
  name: 'module_${i}',
  values: [${Array.from({ length: 10 }, (_, k) => i * 10 + k).join(', ')}]
};
`;
  fs.writeFileSync(path.join(outputDir, `unrelated_${i}.js`), content);
}

// Generate main entry that imports everything
let mainContent = `// Main entry - imports all modules\n`;

// Import the final constant
mainContent += `import { CONST_${CONSTANT_CHAIN_DEPTH - 1} } from './const_${CONSTANT_CHAIN_DEPTH - 1}.js';\n`;

// Import all unrelated modules (just one export each to keep main.js manageable)
for (let i = 0; i < UNRELATED_MODULE_COUNT; i++) {
  mainContent += `import { compute_${i} } from './unrelated_${i}.js';\n`;
}

// Use the constant in a way that benefits from inlining
mainContent += `
// This condition should be eliminated when CONST is inlined as true
if (!CONST_${CONSTANT_CHAIN_DEPTH - 1}) {
  console.log('This should be eliminated by DCE');
}

// Use unrelated modules to prevent tree-shaking them
let sum = 0;
`;

for (let i = 0; i < UNRELATED_MODULE_COUNT; i++) {
  mainContent += `sum += compute_${i}(${i});\n`;
}

mainContent += `
console.log('Sum:', sum);
`;

fs.writeFileSync(path.join(outputDir, 'main.js'), mainContent);

console.log(`Generated benchmark with:`);
console.log(`  - ${CONSTANT_CHAIN_DEPTH} constant chain modules`);
console.log(`  - ${UNRELATED_MODULE_COUNT} unrelated modules`);
console.log(`  - 1 main entry module`);
console.log(`  - Total: ${CONSTANT_CHAIN_DEPTH + UNRELATED_MODULE_COUNT + 1} modules`);

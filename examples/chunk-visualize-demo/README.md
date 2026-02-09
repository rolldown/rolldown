# Chunk Visualization Example

This example demonstrates how to use Rolldown's chunk visualization feature to analyze your bundle composition.

## What is Chunk Visualization?

The chunk visualization plugin generates a JSON file containing detailed information about:

- **Chunks**: All output chunks, their sizes, types, and relationships
- **Modules**: All modules in your bundle and their importers
- **Dependencies**: Import relationships between chunks (static/dynamic)
- **Reachability**: All modules reachable from each entry point

This data can be used to:

- Understand your bundle composition
- Identify optimization opportunities
- Debug code splitting issues
- Track bundle metrics over time

## Running the Example

```bash
# Install dependencies
pnpm install

# Build the project
pnpm run build
```

This will generate:

- `dist/` - Your bundled output files
- `dist/analyze-data.json` - The chunk visualization data

## Configuration

Enable chunk visualization in your `rolldown.config.js`:

```javascript
import { chunkVisualizePlugin } from 'rolldown/experimental';

export default {
  plugins: [
    // Default: generates 'analyze-data.json'
    chunkVisualizePlugin(),

    // Or with custom filename:
    // chunkVisualizePlugin({
    //   fileName: 'bundle-analysis.json'
    // })
  ],
};
```

## JSON Format

The generated JSON contains (`timestamp` is milliseconds since Unix epoch):

```typescript
{
  "meta": {
    "bundler": "rolldown",
    "version": "0.1.0",
    "timestamp": 1705314645123
  },
  "chunks": [
    {
      "id": "chunk-main",
      "name": "main-abc123.js",
      "size": 45230,
      "type": "static-entry",  // or "dynamic-entry" or "common"
      "moduleIndices": [0, 1, 2],
      "entryModule": 0,
      "imports": [
        {
          "targetChunkIndex": 1,
          "type": "static"  // or "dynamic"
        }
      ],
      "reachableModuleIndices": [0, 1, 2, 3, 4]
    }
  ],
  "modules": [
    {
      "id": "mod-0",
      "path": "src/main.js",
      "size": 3450,
      "importers": [1, 2]
    }
  ]
}
```

## Analyzing the Data

### Using Node.js

```javascript
import fs from 'fs';

const data = JSON.parse(fs.readFileSync('dist/analyze-data.json', 'utf-8'));

// Find largest chunks
const largest = data.chunks.sort((a, b) => b.size - a.size).slice(0, 5);

console.log('Top 5 largest chunks:');
largest.forEach((chunk) => {
  console.log(`${chunk.name}: ${(chunk.size / 1024).toFixed(2)} KB`);
});

// Find duplicated modules
const moduleToChunks = new Map();
data.chunks.forEach((chunk, idx) => {
  chunk.moduleIndices?.forEach((modIdx) => {
    if (!moduleToChunks.has(modIdx)) {
      moduleToChunks.set(modIdx, []);
    }
    moduleToChunks.get(modIdx).push(idx);
  });
});

const duplicated = Array.from(moduleToChunks.entries())
  .filter(([_, chunks]) => chunks.length > 1)
  .map(([modIdx, chunks]) => ({
    module: data.modules[modIdx],
    chunks: chunks.map((i) => data.chunks[i].name),
  }));

if (duplicated.length > 0) {
  console.log('\nDuplicated modules:');
  duplicated.forEach(({ module, chunks }) => {
    console.log(`${module.path} appears in ${chunks.length} chunks`);
  });
}
```

### Visualization Tools

Upload the generated JSON to the visualizer:

https://iwanabethatguy.github.io/chunk-visualize/

## Features Demonstrated

This example demonstrates:

- ✅ Static entry chunks
- ✅ Dynamic imports
- ✅ Common/shared chunks
- ✅ Module dependencies
- ✅ Import relationships

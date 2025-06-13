# @rolldown/wasip2

> Rolldown bundler with WASI Preview 2 (wasip2) support

This package provides a build of Rolldown that uses the WebAssembly System Interface (WASI) Preview 2 and WebAssembly Component Model.

## Requirements

To use this package, you'll need a runtime that supports:
- WebAssembly Component Model
- WASI Preview 2 

## Installation

```sh
npm install @rolldown/wasip2
```

## Usage

```js
import { init, bundle, version } from '@rolldown/wasip2';

// Initialize the WASI instance
await init();

// Get the Rolldown version
console.log(`Rolldown version: ${version()}`);

// Bundle with Rolldown
const result = await bundle({
  input: {
    main: 'src/index.js'
  },
  output: {
    dir: 'dist'
  }
});

console.log('Bundle complete:', result);
```

## Features

- Full support for WebAssembly Component Model
- Optimized for WASI Preview 2
- Same API as the native Rolldown bundler

## License

MIT 
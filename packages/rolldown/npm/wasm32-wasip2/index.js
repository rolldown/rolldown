'use strict';

const path = require('path');
const fs = require('fs');

// Path to the wasm file
const wasmPath = path.join(__dirname, 'binding.wasm');

// Check if the binary exists
if (!fs.existsSync(wasmPath)) {
  throw new Error(
    `The Rolldown WASI Preview 2 binary is not installed. Make sure the binding.wasm file exists in ${__dirname}`
  );
}

// Load the wasm file (not actually instantiating for this test)
module.exports = {
  // This is just a mock for testing purposes
  version: function() {
    return '0.1.0-wasip2-test';
  },
  
  // Mock bundle function
  bundle: function(options) {
    return JSON.stringify({
      success: true,
      version: '0.1.0-wasip2-test',
      options: typeof options === 'string' ? JSON.parse(options) : options
    });
  }
}; 
#!/usr/bin/env node
import('../dist/index.js').then(({ serveLegacy, serve }) => {
  if (process.env.USE_LEGACY) {
    serveLegacy();
  } else {
    serve();
  }
});

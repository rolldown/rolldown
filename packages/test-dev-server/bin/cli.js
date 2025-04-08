#!/usr/bin/env node
import('../dist/index.js').then(({ serve }) => {
  serve();
});

#!/usr/bin/env node
import('../dist/index.js').then(({ serve, serveNew }) => {
  if (process.env.USE_NEW_DEV_SERVER) {
    serveNew();
  } else {
    serve();
  }
});

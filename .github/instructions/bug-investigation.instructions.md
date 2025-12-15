---
applyTo: '**'
---

# Bug Investigation Guide

## Decoding REPL URLs

When users share REPL URLs (e.g., `https://repl.rolldown.rs/#<hash>`), you
can decode them to see the configuration and code.

To decode a REPL URL:
1. Extract the hash from the URL (everything after the `#` symbol)
2. Use the following command to decode it:

```bash
node -e "const data = '<input hash>'; const zlib = require('zlib'); \
  console.log(zlib.unzipSync(Buffer.from(data, 'base64')).toString('utf-8'))"
```

Replace `<input hash>` with the actual hash from the REPL URL.

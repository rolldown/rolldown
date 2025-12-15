---
applyTo: '**'
---

# Bug Investigation Guide

To decode the REPL URL, you can use the following command:

```bash
node -e "const data = '<input hash>'; const zlib = require('zlib'); console.log(zlib.unzipSync(Buffer.from(data, 'base64')).toString('utf-8'))"
```

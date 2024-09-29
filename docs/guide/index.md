# Getting Started

## Installation

```json [package.json]
{
  "name": "project",
  "type": "module",
  "scripts": {
    "build": "rolldown -c"
  },
  "devDependencies": {
    "rolldown": "nightly"
  }
}
```

```js [rolldown.config.js]
import { defineConfig } from 'rolldown'

export default defineConfig({
  input: 'src/main.mjs',
})
```

### Versions

- `latest`
- `nightly` - published nightly
- `https://pkg.pr.new/rolldown@45f463a` - each commit on main branch published to [pkg.pr.new](https://pkg.pr.new)

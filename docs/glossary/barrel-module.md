# Barrel Module

A barrel module is a module that re-exports functionality from other modules, commonly used to create a cleaner public API for a package or directory:

```js
// components/index.js (barrel module)
export { Button } from './Button';
export { Card } from './Card';
export { Modal } from './Modal';
export { Tabs } from './Tabs';
// ... dozens more components
```

This allows consumers to import from a single entry point:

```js
import { Button, Card } from './components';
```

However, barrel modules can cause performance issues because bundlers traditionally need to compile all re-exported modules, even if only a few are actually used. See [Lazy Barrel Optimization](/in-depth/lazy-barrel-optimization) for how Rolldown addresses this.

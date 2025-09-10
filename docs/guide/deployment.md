# Deployment

This page documents some deployment-related information about Rolldown-bundled applications.

## Zephyr Cloud

[Zephyr Cloud](https://zephyr-cloud.io/) is a zero-config deployment platform that integrates directly into your build process and provides global edge distribution for federated applications.

Zephyr provides first-class support for Rolldown, allowing you to deploy your applications with minimal configuration.

### How to deploy

Follow the steps in [zephyr-rolldown-plugin](https://npmjs.com/package/zephyr-rolldown-plugin).

```ts
import { defineConfig } from 'rolldown';
import { withZephyr } from 'zephyr-rolldown-plugin';

export default defineConfig({
  input: 'src/main.tsx',
  plugins: [
    // ... other plugins
    withZephyr(),
  ],
});
```

During the build process, your application will be automatically deployed and you'll receive a deployment URL.

Zephyr Cloud handles asset optimization, global CDN distribution, module federation setup, and provides automatic rollback capabilities.

Start for free today at [zephyr-cloud.io](https://zephyr-cloud.io/).

import { defineScenario } from './scenario';

// Mirrors rolldown-starter-stackblitz: one self-contained package, no separate binding.
defineScenario({
  overlay: 'tests/fixtures/browser',
  subject: 'packed @rolldown/browser',
  build: 'pnpm run build',
});

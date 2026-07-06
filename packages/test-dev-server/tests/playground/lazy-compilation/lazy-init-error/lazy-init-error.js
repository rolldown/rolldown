// Fixture for the lazy-init-error scenario: a lazily imported module that throws
// while initializing. See ./setup.js for the behavior the lazy proxy must give.
throw new Error('boom during lazy init');

import { overlayClientScript } from './error-overlay.js';

/**
 * The "Bundling in progress" page, ported from Vite full-bundle mode
 * (`indexHtml.ts::generateFallbackHtml`). Served by the index-html middleware
 * when the bundle output is not ready yet or a stale regeneration was just
 * triggered.
 *
 * Vite inlines its full HMR runtime here (via `getHmrImplementation`) so the
 * spinner holds a live connection and reloads when output is ready. We inline
 * the shared overlay client instead: it reloads on `hmr:reload` (sent on
 * initial-build completion, stale regeneration, and error recovery) AND renders
 * the build error if the initial build failed — so a broken initial build is
 * explained rather than spinning forever.
 */
export function generateFallbackHtml(): string {
  return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>Bundling in progress</title>
  <script type="module">${overlayClientScript()}</script>
  <style>
    :root {
      --page-bg: #ffffff;
      --text-color: #1d1d1f;
      --spinner-track: #f5f5f7;
      --spinner-accent: #0071e3;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --page-bg: #1e1e1e;
        --text-color: #f5f5f5;
        --spinner-track: #424242;
      }
    }

    body {
      margin: 0;
      min-height: 100vh;
      display: flex;
      background-color: var(--page-bg);
      color: var(--text-color);
    }

    .container {
      margin: auto;
      padding: 2rem;
      text-align: center;
      border-radius: 1rem;
    }

    .spinner {
      width: 3rem;
      height: 3rem;
      margin: 2rem auto;
      border: 3px solid var(--spinner-track);
      border-top-color: var(--spinner-accent);
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }

    @keyframes spin { to { transform: rotate(360deg) } }
  </style>
</head>
<body>
  <div class="container">
    <h1>Bundling in progress</h1>
    <p>The page will automatically reload when ready.</p>
    <div class="spinner"></div>
  </div>
</body>
</html>
`;
}

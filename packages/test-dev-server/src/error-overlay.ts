/**
 * Client-side build-error overlay, served by the dev server.
 *
 * This is the test-dev-server's stand-in for Vite's client error overlay. The
 * shared rolldown HMR runtime (`crates/rolldown_plugin_hmr/.../runtime-extra-dev-default.js`)
 * is intentionally NOT modified — instead this snippet is injected into the
 * served HTML (the generated index.html AND the "Bundling in progress"
 * fallback). It runs as its own lightweight websocket client alongside the
 * rolldown HMR runtime: it renders `{ type: 'error' }` messages (broadcast on
 * every build break and replayed on reconnect, so the overlay survives a
 * refresh), clears on a successful `hmr:update`, and reloads on `hmr:reload`
 * (which doubles as the spinner's reload mechanism).
 */
export function overlayClientScript(): string {
  return /* js */ `
const OVERLAY_ID = 'rolldown-error-overlay';
function clearOverlay() {
  const el = document.getElementById(OVERLAY_ID);
  if (el) el.remove();
}
function showOverlay(err) {
  clearOverlay();
  const overlay = document.createElement('div');
  overlay.id = OVERLAY_ID;
  overlay.setAttribute(
    'style',
    'position:fixed;inset:0;z-index:99999;margin:0;background:rgba(0,0,0,0.85);'
      + 'color:#ff5555;font-family:monospace;font-size:14px;line-height:1.5;'
      + 'padding:24px;white-space:pre-wrap;overflow:auto;',
  );
  // Build a readable report from the \`prepareError\` payload, keeping the
  // message visible even when the cleaned stack only holds internal frames.
  const parts = [err.message];
  if (err.plugin) parts.push('Plugin: ' + err.plugin);
  if (err.id) {
    const loc = err.loc ? ':' + err.loc.line + ':' + err.loc.column : '';
    parts.push('File: ' + err.id + loc);
  }
  if (err.frame) parts.push(err.frame);
  if (err.stack) parts.push(err.stack);
  overlay.textContent = parts.join('\\n');
  document.body.appendChild(overlay);
}
const clientId = crypto.randomUUID();
const socket = new WebSocket('ws://' + location.host + '?clientId=' + clientId);
// The "[test-dev-server]" console markers below exist for the browser test
// harness: it asserts ordered log sequences via untilBrowserLogAfter instead
// of only DOM-polling. See meta/design/dev-server-test-harness.md.
socket.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.type === 'error') {
    showOverlay(data.err);
    console.log('[test-dev-server] error overlay shown: ' + data.err.message);
  } else if (data.type === 'build:ok') {
    clearOverlay();
    console.log('[test-dev-server] build ok');
  } else if (data.type === 'hmr:update') {
    clearOverlay();
  } else if (data.type === 'hmr:reload') {
    location.reload();
  }
};
// Post-apply HMR marker. The runtime logs BEFORE a patch executes (loading is
// async), so wrap applyUpdates to log after it ran. This script tag is
// injected last in the document, so the entry module has already installed
// the runtime; the guard covers the spinner page (no runtime there).
const runtime = globalThis.__rolldown_runtime__;
if (runtime && typeof runtime.applyUpdates === 'function') {
  const originalApplyUpdates = runtime.applyUpdates;
  runtime.applyUpdates = function (boundaries) {
    const result = originalApplyUpdates.call(this, boundaries);
    console.log(
      '[test-dev-server] hot updated: ' + boundaries.map((b) => b[0]).join(', '),
    );
    return result;
  };
}
`;
}

/** Inject the overlay client script before `</body>` (or append it). */
export function injectOverlayScript(html: string): string {
  const tag = `<script type="module">${overlayClientScript()}</script>`;
  if (html.includes('</body>')) {
    return html.replace('</body>', `${tag}\n</body>`);
  }
  return html + tag;
}

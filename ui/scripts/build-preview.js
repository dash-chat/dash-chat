/**
 * Post-build script that injects a preview bootstrap into index.html.
 *
 * When the page loads as the top-level window (not inside an iframe), the
 * bootstrap replaces the document with a toolbar + iframe shell that loads
 * the app via `/?__embed`. When `?__embed` is present, the bootstrap is
 * skipped and SvelteKit boots normally.
 *
 * This keeps the toolbar DOM fully isolated from the app, so fixed-position
 * elements (FAB, tooltips, banners) are never obscured by the toolbar.
 *
 * Usage:  node ui/scripts/build-preview.js
 */

import { readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const buildDir = join(__dirname, '..', 'build');
const indexPath = join(buildDir, 'index.html');
const fallbackPath = join(buildDir, '200.html');

const TOOLBAR_HEIGHT = 48;

// Only the HTML markup goes into document.write() — no nested <script> tags.
// Event listeners are attached afterwards in the bootstrap script itself.
const wrapperHtml = /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Dash Chat Preview</title>
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
  html, body { height: 100%; overflow: hidden; font-family: -apple-system, 'Segoe UI', Roboto, sans-serif; }
  #toolbar {
    height: ${TOOLBAR_HEIGHT}px;
    background: #1a1a2e;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    flex-shrink: 0;
  }
  #toolbar button {
    background: #2a2a4a;
    color: #e0e0e0;
    border: 1px solid #3a3a5a;
    border-radius: 4px;
    padding: 4px 14px;
    font-size: 12px;
    cursor: pointer;
    font-family: inherit;
    white-space: nowrap;
    transition: background 0.15s;
  }
  #toolbar button:hover { background: #3a3a5a; }
  #toolbar button.reset { background: #4a2020; border-color: #6a3030; }
  #toolbar button.reset:hover { background: #6a3030; }
  #toolbar button.danger { background: #6a2020; border-color: #8a3030; }
  #toolbar button.danger:hover { background: #8a3030; }
  #frame-wrap {
    flex: 1;
    display: flex;
    justify-content: center;
    background: #f0f0f0;
    overflow: hidden;
  }
  body.dark #frame-wrap { background: #222; }
  #app-frame { border: none; width: 100%; height: 100%; }
  #app-frame.mobile {
    width: 390px;
    max-width: 100%;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.1), 0 4px 24px rgba(0,0,0,0.15);
  }
  body { display: flex; flex-direction: column; }
</style>
</head>
<body>
<div id="toolbar">
  <button id="btn-theme">Material</button>
  <button id="btn-dark">Light</button>
  <button id="btn-layout">Desktop</button>
  <button id="btn-updater">Show Update</button>
  <button id="btn-reset" class="reset">Reset</button>
  <button id="btn-wipe" class="danger">Wipe</button>
</div>
<div id="frame-wrap">
  <iframe id="app-frame" src="/?__embed"></iframe>
</div>
</body>
</html>`;

// The bootstrap script: replaces the page with the wrapper HTML, then attaches
// toolbar event listeners directly (no nested <script> needed).
const bootstrapScript = /* html */ `<script>
(function() {
  if (window.self !== window.top || location.search.indexOf('__embed') !== -1) return;

  document.open();
  document.write(${JSON.stringify(wrapperHtml)});
  document.close();

  var frame = document.getElementById('app-frame');
  var state = { theme: 'material', dark: false, mobile: false, updater: false };

  function send(type, payload) {
    if (frame.contentWindow) frame.contentWindow.postMessage({ type: type, payload: payload }, '*');
  }

  document.getElementById('btn-theme').addEventListener('click', function () {
    state.theme = state.theme === 'ios' ? 'material' : 'ios';
    this.textContent = state.theme === 'ios' ? 'iOS' : 'Material';
    send('theme-change', { theme: state.theme });
  });

  document.getElementById('btn-dark').addEventListener('click', function () {
    state.dark = !state.dark;
    this.textContent = state.dark ? 'Dark' : 'Light';
    document.body.classList.toggle('dark', state.dark);
    send('set-dark-mode', state.dark);
  });

  document.getElementById('btn-layout').addEventListener('click', function () {
    state.mobile = !state.mobile;
    this.textContent = state.mobile ? 'Mobile' : 'Desktop';
    frame.classList.toggle('mobile', state.mobile);
    send('set-wide-screen', !state.mobile);
  });

  document.getElementById('btn-updater').addEventListener('click', function () {
    state.updater = !state.updater;
    this.textContent = state.updater ? 'Hide Update' : 'Show Update';
    send('simulate-update', state.updater ? 'downloading' : 'idle');
  });

  document.getElementById('btn-reset').addEventListener('click', function () { send('reset'); });
  document.getElementById('btn-wipe').addEventListener('click', function () { send('wipe'); });
})();
</script>`;

// Inject bootstrap right after <head> so it runs before any SvelteKit scripts
const indexHtml = readFileSync(indexPath, 'utf-8');
const injected = indexHtml.replace('<head>', '<head>' + bootstrapScript);
writeFileSync(indexPath, injected, 'utf-8');

// SPA fallback — copy the patched index.html so all routes get the bootstrap
copyFileSync(indexPath, fallbackPath);

console.log('Preview bootstrap injected into build/index.html');

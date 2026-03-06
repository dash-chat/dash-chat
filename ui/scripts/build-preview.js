/**
 * Post-build script that generates preview.html — a shell page with an inline
 * toolbar at the top and the Svelte app rendered inside an <iframe>.
 *
 * This keeps the toolbar DOM fully isolated from the app, so fixed-position
 * elements (FAB, tooltips, banners) are never obscured or broken by the toolbar.
 *
 * Usage:  node ui/scripts/build-preview.js
 * The output lands in ui/build/preview.html.
 */

import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const outPath = join(__dirname, '..', 'build', 'preview.html');

const TOOLBAR_HEIGHT = 48;

const html = /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Dash Chat Preview</title>
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
  html, body { height: 100%; overflow: hidden; font-family: -apple-system, 'Segoe UI', Roboto, sans-serif; }

  /* --- Toolbar --- */
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

  /* --- Iframe wrapper --- */
  #frame-wrap {
    flex: 1;
    display: flex;
    justify-content: center;
    background: #f0f0f0;
    overflow: hidden;
  }
  body.dark #frame-wrap { background: #222; }

  #app-frame {
    border: none;
    width: 100%;
    height: 100%;
  }
  #app-frame.mobile {
    width: 390px;
    max-width: 100%;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.1), 0 4px 24px rgba(0,0,0,0.15);
  }

  /* Vertical stack */
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
  <iframe id="app-frame" src="/index.html"></iframe>
</div>

<script>
(function () {
  var frame = document.getElementById('app-frame');
  var state = {
    theme: 'material',
    dark: false,
    mobile: false,
    updater: false,
  };

  function send(type, payload) {
    if (frame.contentWindow) {
      frame.contentWindow.postMessage({ type: type, payload: payload }, '*');
    }
  }

  // --- Theme ---
  document.getElementById('btn-theme').addEventListener('click', function () {
    state.theme = state.theme === 'ios' ? 'material' : 'ios';
    this.textContent = state.theme === 'ios' ? 'iOS' : 'Material';
    send('theme-change', { theme: state.theme });
  });

  // --- Dark mode ---
  document.getElementById('btn-dark').addEventListener('click', function () {
    state.dark = !state.dark;
    this.textContent = state.dark ? 'Dark' : 'Light';
    document.body.classList.toggle('dark', state.dark);
    send('set-dark-mode', state.dark);
  });

  // --- Layout ---
  document.getElementById('btn-layout').addEventListener('click', function () {
    state.mobile = !state.mobile;
    this.textContent = state.mobile ? 'Mobile' : 'Desktop';
    frame.classList.toggle('mobile', state.mobile);
    send('set-wide-screen', !state.mobile);
  });

  // --- Updater ---
  document.getElementById('btn-updater').addEventListener('click', function () {
    state.updater = !state.updater;
    this.textContent = state.updater ? 'Hide Update' : 'Show Update';
    send('simulate-update', state.updater ? 'downloading' : 'idle');
  });

  // --- Reset ---
  document.getElementById('btn-reset').addEventListener('click', function () {
    send('reset');
  });

  // --- Wipe ---
  document.getElementById('btn-wipe').addEventListener('click', function () {
    send('wipe');
  });
})();
</script>
</body>
</html>`;

writeFileSync(outPath, html, 'utf-8');
console.log(`preview.html written to ${outPath}`);

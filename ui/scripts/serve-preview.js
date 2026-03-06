/**
 * Builds the app with the preview wrapper injected, then serves it locally.
 *
 * Usage:  node scripts/serve-preview.js [--port 4173]
 */

import { execSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, statSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const buildDir = join(__dirname, '..', 'build');

const args = process.argv.slice(2);
const portIdx = args.indexOf('--port');
const port = portIdx !== -1 ? Number(args[portIdx + 1]) : 4173;

// 1. Build
console.log('Building...');
execSync('vite build', { cwd: join(__dirname, '..'), stdio: 'inherit' });

// 2. Inject preview bootstrap
console.log('Injecting preview bootstrap...');
execSync('node scripts/build-preview.js', { cwd: join(__dirname, '..'), stdio: 'inherit' });

// 3. Serve
const MIME = {
	'.html': 'text/html',
	'.js': 'application/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.png': 'image/png',
	'.jpg': 'image/jpeg',
	'.jpeg': 'image/jpeg',
	'.svg': 'image/svg+xml',
	'.ico': 'image/x-icon',
	'.woff': 'font/woff',
	'.woff2': 'font/woff2',
	'.ttf': 'font/ttf',
	'.webp': 'image/webp',
	'.webm': 'video/webm',
	'.mp4': 'video/mp4',
};

function tryFile(filePath) {
	try {
		const stat = statSync(filePath);
		if (stat.isFile()) return readFileSync(filePath);
	} catch {}
	return null;
}

const server = createServer((req, res) => {
	const url = new URL(req.url, `http://localhost:${port}`);
	let pathname = url.pathname;

	// Try exact file
	let body = tryFile(join(buildDir, pathname));

	// Try with .html extension
	if (!body && !extname(pathname)) {
		body = tryFile(join(buildDir, pathname + '.html'));
	}

	// Try index.html in directory
	if (!body) {
		body = tryFile(join(buildDir, pathname, 'index.html'));
	}

	// SPA fallback → 200.html
	if (!body) {
		body = tryFile(join(buildDir, '200.html'));
	}

	if (body) {
		const ext = extname(pathname) || '.html';
		res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' });
		res.end(body);
	} else {
		res.writeHead(404);
		res.end('Not found');
	}
});

server.listen(port, () => {
	console.log(`\n  Preview: http://localhost:${port}/\n`);
});

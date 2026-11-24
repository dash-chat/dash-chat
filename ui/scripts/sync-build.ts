// Mirrors the adapter's staged output into build/, touching only files whose
// bytes changed: cargo relinks the app whenever an embedded frontend file's
// mtime moves, and a plain rewrite would relink on every build.
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	rmSync,
	statSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const stage = fileURLToPath(new URL('../.build-stage/', import.meta.url));
const build = fileURLToPath(new URL('../build/', import.meta.url));

function entriesUnder(dir: string): { relative: string; isFile: boolean }[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir, { recursive: true, withFileTypes: true }).map(
		entry => ({
			relative: path.relative(dir, path.join(entry.parentPath, entry.name)),
			isFile: entry.isFile(),
		}),
	);
}

function isFile(file: string): boolean {
	return existsSync(file) && statSync(file).isFile();
}

const staged = new Set(
	entriesUnder(stage)
		.filter(entry => entry.isFile)
		.map(entry => entry.relative),
);
const stagedDirs = new Set([...staged].map(file => path.dirname(file)));

for (const { relative, isFile: wasFile } of entriesUnder(build)) {
	const keep = wasFile
		? staged.has(relative)
		: [...stagedDirs].some(
				dir => dir === relative || dir.startsWith(`${relative}/`),
			);
	if (!keep)
		rmSync(path.join(build, relative), { recursive: true, force: true });
}
for (const relative of staged) {
	const from = path.join(stage, relative);
	const to = path.join(build, relative);
	if (isFile(to) && readFileSync(from).equals(readFileSync(to))) continue;
	mkdirSync(path.dirname(to), { recursive: true });
	copyFileSync(from, to);
}

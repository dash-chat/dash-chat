import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

type Span = {
	file_name: string;
	line_start: number;
	line_end: number;
	is_primary: boolean;
};

type CargoMessage = {
	reason: string;
	message?: { message: string; code: { code: string } | null; spans: Span[] };
};

type Diagnostic = { code: string; text: string; span: Span };

type Candidate = {
	file: string;
	line: number;
	endLine: number;
	lines: number;
	complexity: number;
	deepBlocks: number;
	mixedOperations: number;
	score: number;
};

const MIXED_OPERATIONS_SCALE = 5;
const SECONDS_PER_DAY = 86400;

/** Runs a cargo lint command and returns its deduplicated diagnostics. */
function lintDiagnostics(args: string[], env: NodeJS.ProcessEnv): Diagnostic[] {
	const stdout = execFileSync('cargo', [...args, '--message-format=json'], {
		env,
		encoding: 'utf8',
		maxBuffer: 1024 ** 3,
		stdio: ['ignore', 'pipe', 'inherit'],
	});
	const diagnostics = new Map<string, Diagnostic>();
	for (const line of stdout.split('\n')) {
		if (!line.startsWith('{')) continue;
		const { reason, message }: CargoMessage = JSON.parse(line);
		const span = message?.spans.find(s => s.is_primary);
		if (reason !== 'compiler-message' || !message?.code || !span) continue;
		const code = message.code.code;
		diagnostics.set(`${code}:${span.file_name}:${span.line_start}`, {
			code,
			text: message.message,
			span,
		});
	}
	return [...diagnostics.values()];
}

function clippyThreshold(name: string): number {
	const match = readFileSync('clippy.toml', 'utf8').match(
		new RegExp(`^${name}\\s*=\\s*(\\d+)`, 'm'),
	);
	if (!match) throw new Error(`${name} is not set in clippy.toml`);
	return Number(match[1]);
}

/** Clippy diagnostics with the size thresholds at zero, so every function reports its metrics. */
function measureWithClippy(nestingThreshold: number): Diagnostic[] {
	const configDir = mkdtempSync(join(tmpdir(), 'clippy-metrics-'));
	writeFileSync(
		join(configDir, 'clippy.toml'),
		[
			'cognitive-complexity-threshold = 0',
			'too-many-lines-threshold = 0',
			`excessive-nesting-threshold = ${nestingThreshold}`,
		].join('\n'),
	);
	return lintDiagnostics(['clippy', '--workspace'], {
		...process.env,
		CLIPPY_CONF_DIR: configDir,
	});
}

/** The first parenthesised number in a lint message, e.g. 34 in "too many lines (34/0)". */
function reportedValue(text: string): number {
	return Number(text.match(/\((\d+)/)?.[1] ?? 0);
}

function within(span: Span, file: string, start: number, end: number): boolean {
	return (
		span.file_name === file && span.line_start >= start && span.line_start <= end
	);
}

function buildCandidates(clippy: Diagnostic[], custom: Diagnostic[]): Candidate[] {
	const complexityThreshold = clippyThreshold('cognitive-complexity-threshold');
	const linesThreshold = clippyThreshold('too-many-lines-threshold');
	const functions = clippy
		.filter(d => d.code === 'clippy::too_many_lines')
		.sort(
			(a, b) =>
				a.span.file_name.localeCompare(b.span.file_name) ||
				a.span.line_start - b.span.line_start,
		);

	return functions.map((fn, index) => {
		const file = fn.span.file_name;
		const line = fn.span.line_start;
		const next = functions[index + 1];
		const endLine =
			next?.span.file_name === file ? next.span.line_start - 1 : Infinity;
		const inSignature = (d: Diagnostic) =>
			within(d.span, file, line, fn.span.line_end);
		const inFunction = (d: Diagnostic) => within(d.span, file, line, endLine);

		const lines = reportedValue(fn.text);
		const complexity = reportedValue(
			clippy.find(
				d => d.code === 'clippy::cognitive_complexity' && inSignature(d),
			)?.text ?? '',
		);
		const deepBlocks = clippy.filter(
			d => d.code === 'clippy::excessive_nesting' && inFunction(d),
		).length;
		const mixed = custom.find(
			d => d.code === 'mixed_abstraction' && inSignature(d),
		);
		const mixedOperations = Number(
			mixed?.text.match(/(\d+) low-level operations/)?.[1] ?? 0,
		);
		const score =
			complexity / complexityThreshold +
			lines / linesThreshold +
			deepBlocks +
			mixedOperations / MIXED_OPERATIONS_SCALE;
		return {
			file,
			line,
			endLine,
			lines,
			complexity,
			deepBlocks,
			mixedOperations,
			score,
		};
	});
}

function daysUntouched({ file, line, endLine }: Candidate): number {
	const range = `${line},${Number.isFinite(endLine) ? endLine : ''}`;
	const blame = execFileSync(
		'git',
		['blame', '-w', '-M', '--line-porcelain', '-L', range, file],
		{ encoding: 'utf8', maxBuffer: 1024 ** 3 },
	);
	const commitTimes = [...blame.matchAll(/^committer-time (\d+)$/gm)].map(m =>
		Number(m[1]),
	);
	return Math.floor(
		(Date.now() / 1000 - Math.max(...commitTimes)) / SECONDS_PER_DAY,
	);
}

function functionName({ file, line, endLine }: Candidate): string {
	const source = readFileSync(file, 'utf8').split('\n');
	const signature = source.slice(line - 1, Math.min(endLine, line + 5)).join(' ');
	return signature.match(/\bfn\s+(\w+)/)?.[1] ?? '';
}

const { values } = parseArgs({
	options: {
		'min-days': { type: 'string', default: '21' },
		top: { type: 'string', default: '10' },
	},
});
const minDays = Number(values['min-days']);
const top = Number(values.top);

const shallow = execFileSync('git', ['rev-parse', '--is-shallow-repository'], {
	encoding: 'utf8',
});
if (shallow.trim() === 'true') {
	console.error(
		'warning: shallow clone, so "days untouched" is capped at the oldest fetched commit',
	);
}

const clippy = measureWithClippy(clippyThreshold('excessive-nesting-threshold'));
const custom = lintDiagnostics(['dylint', '--all', '--workspace', '--'], process.env);
const ranked = buildCandidates(clippy, custom)
	.filter(c => c.score >= 1)
	.sort((a, b) => b.score - a.score);

const stale: Record<string, string | number>[] = [];
for (const candidate of ranked) {
	if (stale.length >= top) break;
	const days = daysUntouched(candidate);
	if (days < minDays) continue;
	stale.push({
		score: candidate.score.toFixed(1),
		complexity: candidate.complexity,
		lines: candidate.lines,
		'deep blocks': candidate.deepBlocks,
		'mixed ops': candidate.mixedOperations,
		'days untouched': days,
		function: functionName(candidate),
		location: `${candidate.file}:${candidate.line}`,
	});
}
console.table(stale);

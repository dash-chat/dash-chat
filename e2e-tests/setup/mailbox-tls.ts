/**
 * TLS in front of the e2e mailbox: agents → toxiproxy → stunnel → mailbox.
 * The app's connect timeout covers the TLS handshake, so with TLS behind the
 * proxy a degraded link slows connection setup the way a real network does;
 * over plain HTTP the connection is open the moment the proxy accepts it.
 * E2E builds trust the committed test CA the mailbox client compiles in.
 */
import {
	type ChildProcess,
	execFileSync,
	execSync,
	spawn,
} from 'node:child_process';
import { X509Certificate } from 'node:crypto';
import {
	closeSync,
	mkdirSync,
	openSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import { isIP } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from './agent-logger';
import { allocateFreePort } from './allocate-port';
import { waitForPortListening } from './wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const CA_DIR = path.join(ROOT, 'crates', 'mailbox-client', 'e2e-test-ca');
// DER because the app's e2e build embeds this file and native-tls only parses
// PEM on macOS; the PEM openssl and node want is derived from it.
const CA_CERT_PEM = new X509Certificate(
	readFileSync(path.join(CA_DIR, 'ca.der')),
).toString();
const CA_KEY = path.join(CA_DIR, 'ca-key.pem');
const RUN_DIR = path.join(ROOT, '.dbs', 'e2e', 'mailbox-tls');

function assertStunnelAvailable(): void {
	try {
		execSync('command -v stunnel', { stdio: 'ignore' });
	} catch {
		throw new Error(
			'stunnel not found — every e2e run serves the local mailbox over TLS ' +
				'through it. Run inside the nix dev shell, or install stunnel onto ' +
				'your PATH.',
		);
	}
}

/** Every address an agent may dial the mailbox at: loopback for desktop and
 *  Android (through `adb reverse`), the host's LAN addresses for iPhones, and
 *  whatever `E2E_HOST_IP` pins them to instead. */
function mailboxHostNames(): string[] {
	const lanAddresses = Object.values(os.networkInterfaces())
		.flat()
		.filter(iface => iface !== undefined && iface.family === 'IPv4')
		.map(iface => `IP:${iface!.address}`);
	const pinned = process.env.E2E_HOST_IP;
	if (pinned !== undefined && pinned !== '') {
		lanAddresses.push(isIP(pinned) === 0 ? `DNS:${pinned}` : `IP:${pinned}`);
	}
	return ['DNS:localhost', ...new Set(lanAddresses)];
}

/** A server certificate for this run's host names, signed by the test CA.
 *  Kept within Apple's limits for TLS server certificates (825 days, EKU
 *  serverAuth), which iOS enforces on custom anchors too. */
function issueServerCertificate(): { cert: string; key: string } {
	mkdirSync(RUN_DIR, { recursive: true });
	const key = path.join(RUN_DIR, 'server-key.pem');
	const csr = path.join(RUN_DIR, 'server.csr');
	const cert = path.join(RUN_DIR, 'server.pem');
	const extensions = path.join(RUN_DIR, 'server.ext');
	writeFileSync(
		extensions,
		[
			`subjectAltName=${mailboxHostNames().join(',')}`,
			'basicConstraints=CA:FALSE',
			'keyUsage=critical,digitalSignature',
			'extendedKeyUsage=serverAuth',
		].join('\n'),
	);
	const caCert = path.join(RUN_DIR, 'ca.pem');
	writeFileSync(caCert, CA_CERT_PEM);
	openssl(['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', key]);
	openssl(['req', '-new', '-key', key, '-subj', '/CN=localhost', '-out', csr]);
	openssl([
		'x509',
		'-req',
		'-in',
		csr,
		'-CA',
		caCert,
		'-CAkey',
		CA_KEY,
		// A random serial: `-CAcreateserial` would write a serial file next to the
		// committed CA.
		'-set_serial',
		`0x${Date.now().toString(16)}`,
		'-days',
		'30',
		'-sha256',
		'-extfile',
		extensions,
		'-out',
		cert,
	]);
	return { cert, key };
}

/** Its chatter goes into the thrown error rather than the run's output. */
function openssl(args: string[]): void {
	execFileSync('openssl', args, { stdio: 'pipe' });
}

/** Terminate TLS on a fresh port and forward the plaintext to `upstreamPort`.
 *  stunnel rather than socat: socat pins its key exchange to P-256, so every
 *  client that opens with an X25519 key share, as OpenSSL 3 does, pays a
 *  HelloRetryRequest round trip the production mailbox never asks for. */
export async function startMailboxTls(upstreamPort: number): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
	port: number;
}> {
	assertStunnelAvailable();
	const { cert, key } = issueServerCertificate();
	const port = await allocateFreePort();
	const config = path.join(RUN_DIR, 'stunnel.conf');
	writeFileSync(
		config,
		[
			'foreground = yes',
			'pid =',
			'debug = warning',
			'[mailbox]',
			`accept = 127.0.0.1:${port}`,
			`connect = 127.0.0.1:${upstreamPort}`,
			`cert = ${cert}`,
			`key = ${key}`,
		].join('\n'),
	);
	const logFile = path.join(RUN_DIR, 'stunnel.log');
	const logger = startAgentLogger('mailbox-tls', logFile);
	const logFd = openSync(logFile, 'a');
	const proc = spawn('stunnel', [config], {
		stdio: ['ignore', logFd, logFd],
		detached: true,
		// The checkout-scoped cleanup tells this run's stunnel from another
		// checkout's by this path in its environment.
		env: { ...process.env, E2E_DBS: path.join(ROOT, '.dbs') + path.sep },
	});
	closeSync(logFd);
	await new Promise<void>((resolve, reject) => {
		proc.once('spawn', resolve);
		proc.once('error', err =>
			reject(new Error(`could not start stunnel: ${err.message}`)),
		);
	});
	await waitForPortListening(port);
	console.log(`[mailbox-tls] ready on port ${port} (pid=${proc.pid})`);
	return { proc, logger, port };
}

/** Whether the mailbox at `url` answers /health within `timeoutMs`. Local hubs
 *  serve plain HTTP; only the cloud mailbox sits behind TLS. */
export function mailboxAnswers(
	url: string,
	timeoutMs: number,
): Promise<boolean> {
	const client = new URL(url).protocol === 'https:' ? https : http;
	return new Promise(resolve => {
		const request = client.get(
			`${url}/health`,
			{ ca: CA_CERT_PEM, timeout: timeoutMs },
			response => {
				response.resume();
				const status = response.statusCode ?? 0;
				resolve(status >= 200 && status < 300);
			},
		);
		request.on('timeout', () => {
			request.destroy();
			resolve(false);
		});
		request.on('error', () => resolve(false));
	});
}

import { execFileSync } from 'node:child_process';

const IFCONFIG = '/sbin/ifconfig';
const RESTORE_ARGS = ['lo0', 'alias', '127.0.0.1', '255.0.0.0'];

/**
 * Put 127.0.0.1 back on lo0 when it's missing. Something on some Macs strips
 * it at boot (leaving only 10.10.10.1/24), after which every localhost server
 * the harness starts fails to become ready. Restoring it needs root, so it
 * goes through a passwordless sudo rule for exactly this command.
 */
export function ensureLoopback() {
	if (process.platform !== 'darwin' || hasLoopbackV4()) return;

	console.log('lo0 is missing 127.0.0.1, restoring it');
	try {
		execFileSync('sudo', ['-n', IFCONFIG, ...RESTORE_ARGS], { stdio: 'pipe' });
	} catch {
		throw new Error(
			`lo0 is missing 127.0.0.1 and the harness can't restore it without a password. ` +
				`Allow it once with:\n` +
				`  echo "$USER ALL=(root) NOPASSWD: ${IFCONFIG} ${RESTORE_ARGS.join(' ')}" | sudo tee /etc/sudoers.d/dash-chat-e2e-loopback && sudo chmod 440 /etc/sudoers.d/dash-chat-e2e-loopback`,
		);
	}
	if (!hasLoopbackV4()) {
		throw new Error('lo0 still has no 127.0.0.1 after restoring it');
	}
}

function hasLoopbackV4(): boolean {
	return execFileSync(IFCONFIG, ['lo0'], { encoding: 'utf8' }).includes(
		'inet 127.0.0.1 ',
	);
}

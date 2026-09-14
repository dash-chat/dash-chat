import assert from 'node:assert/strict';
import { test } from 'node:test';

import { wifiNetworks } from './test-env.ts';

function withVar<T>(name: string, value: string | undefined, body: () => T): T {
	const before = process.env[name];
	if (value === undefined) delete process.env[name];
	else process.env[name] = value;
	try {
		return body();
	} finally {
		if (before === undefined) delete process.env[name];
		else process.env[name] = before;
	}
}

function withEnv<T>(value: string | undefined, body: () => T): T {
	return withVar('E2E_WIFI_NETWORKS', value, body);
}

test('no networks when unset or blank', () => {
	assert.deepEqual(withEnv(undefined, wifiNetworks), []);
	assert.deepEqual(withEnv('  ', wifiNetworks), []);
});

test('networks keep their order and allow open ones', () => {
	assert.deepEqual(withEnv('lab-a:pw1, lab-b:, lab-c', wifiNetworks), [
		{ ssid: 'lab-a', passphrase: 'pw1', home: false },
		{ ssid: 'lab-b', passphrase: '', home: false },
		{ ssid: 'lab-c', passphrase: '', home: false },
	]);
});

test('a malformed entry names itself', () => {
	assert.throws(() => withEnv(':pw', wifiNetworks), /':pw'/);
	assert.throws(() => withEnv('a:b:c', wifiNetworks), /'a:b:c'/);
	assert.throws(() => withEnv('a,,b', wifiNetworks), /''/);
});

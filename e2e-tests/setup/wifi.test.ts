import assert from 'node:assert/strict';
import { test } from 'node:test';

import { networkOf } from './wifi.ts';

test('a /24 keeps three octets and clears the last', () => {
	assert.equal(networkOf('192.168.1.16', 24), '192.168.1.0/24');
});

test('a wider mask clears across octets', () => {
	assert.equal(networkOf('10.0.7.90', 16), '10.0.0.0/16');
	assert.equal(networkOf('10.0.3.14', 22), '10.0.0.0/22');
	assert.equal(networkOf('10.0.7.90', 22), '10.0.4.0/22');
});

test('a /32 and no address', () => {
	assert.equal(networkOf('192.168.88.237', 32), '192.168.88.237/32');
	assert.equal(networkOf('', 24), '');
	assert.equal(networkOf('192.168.1.16', 0), '');
	assert.equal(networkOf('192.168.1.16', Number.NaN), '');
});

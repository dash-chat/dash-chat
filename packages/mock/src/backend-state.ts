import type {
	AgentId,
	AllMailboxesConnectionState,
	MailboxConnectionHealth,
	MessageAcks,
} from 'dash-chat-stores';
import Emittery, { type UnsubscribeFunction } from 'emittery';
import { type ReactivePromise, relay } from 'signalium';

// Far future: a success or failure before the page's last resume would read
// as unproven rather than as the state it stands for.
const AFTER_ANY_RESUME = '9999-01-01T00:00:00Z';

export const MOCK_MAILBOX_UP: MailboxConnectionHealth = {
	status: 'Active',
	consecutive_errors: 0,
	next_poll_in_ms: 5000,
	last_success_at: AFTER_ANY_RESUME,
	last_error: null,
};

export const MOCK_MAILBOX_UNCONFIRMED: MailboxConnectionHealth = {
	...MOCK_MAILBOX_UP,
	last_success_at: null,
};

export const MOCK_MAILBOX_DOWN: MailboxConnectionHealth = {
	status: 'Stopped',
	consecutive_errors: 10,
	next_poll_in_ms: 5000,
	last_success_at: null,
	last_error: { at: AFTER_ANY_RESUME, message: 'unreachable' },
};

export interface MockBackend {
	mailboxes: AllMailboxesConnectionState;
	onlinePeers: AgentId[];
	operationsReachMailboxes: boolean;
	/** The backend commands that fail, as the real client spells them. After a
	 * colon comes the `kind` of the error it fails with, for the ones the UI
	 * tells apart: 'leave_group:LastAdmin'. */
	failingCommands: string[];
	/** What other people's devices have acknowledged receiving, per author. */
	messageAcks: MessageAcks;
	/** What the backend commands the UI calls directly answer, by command. */
	commandAnswers: Record<string, boolean | string | Record<string, boolean>>;
	/** Whether photos, files and voice notes fail to load. */
	mediaIsMissing: boolean;
	/** Whether the local hub is kept off the LAN. */
	localHubBlocked: boolean;
}

const AS_THE_APP_STARTS: MockBackend = {
	mailboxes: { cloudMailbox: MOCK_MAILBOX_UP, localMailboxes: [] },
	onlinePeers: [],
	operationsReachMailboxes: true,
	failingCommands: [],
	messageAcks: {},
	commandAnswers: {},
	mediaIsMissing: false,
	localHubBlocked: false,
};

const backendChanges = new Emittery<{ changed: undefined }>();

const STORED_AS = 'mock-backend';

/** What the mock backend reports about the world outside the device. It is
 * kept in storage so that it can be in place before the app starts, and in
 * the session's, which the app does not clear when an account is deleted.
 * Change it with `setMockBackend`, which is what tells the stores. */
export function mockBackend(): MockBackend {
	const stored = sessionStorage.getItem(STORED_AS);
	return {
		...AS_THE_APP_STARTS,
		...(stored === null ? {} : (JSON.parse(stored) as Partial<MockBackend>)),
	};
}

export function setMockBackend(changes: Partial<MockBackend>): void {
	sessionStorage.setItem(
		STORED_AS,
		JSON.stringify({ ...mockBackend(), ...changes }),
	);
	void backendChanges.emit('changed');
}

export function onMockBackendChanged(handler: () => void): UnsubscribeFunction {
	return backendChanges.on('changed', handler);
}

/** What `read` finds in the mock backend, kept up with its changes. */
export function followingTheMockBackend<T>(read: () => T): ReactivePromise<T> {
	return relay<T>(state => {
		state.value = read();
		return onMockBackendChanged(() => {
			state.value = read();
		});
	});
}

/** Throws when `command` is one the mock backend was told fails. */
export function failIfTold(command: string): void {
	const { failingCommands } = mockBackend();
	if (failingCommands.includes(command)) {
		throw new Error('mock failure');
	}
	const withKind = failingCommands.find(failing =>
		failing.startsWith(`${command}:`),
	);
	if (withKind !== undefined) {
		throw { kind: withKind.slice(command.length + 1) };
	}
}

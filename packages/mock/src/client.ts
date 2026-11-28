import blakejs from 'blakejs';
import type {
	GroupsArgs,
	Hash,
	Header,
	LogId,
	LogsClient,
	Operation,
	SimplifiedHeader,
	SimplifiedOperation,
	TopicId,
	VerifyingKey,
} from 'dash-chat-stores';
import Emittery, { type UnsubscribeFunction } from 'emittery';

export function hash<T>(obj: T): Hash {
	return blakejs.blake2bHex(JSON.stringify(obj));
}

export function chatIdForDevices(deviceA: string, deviceB: string): string {
	return `mock-chat:${[deviceA, deviceB].sort().join(':')}`;
}

// One for every client: an operation written as any author reaches whoever
// listens, the way the backend reports the ones it syncs from other devices.
const operationsWritten = new Emittery();

export class LocalStorageLogsClient implements LogsClient<any> {
	constructor(protected _myPubKey: VerifyingKey) {}

	async myPubKey() {
		return this._myPubKey;
	}

	getItems() {
		const items = { ...localStorage };
		return items;
	}

	async getLog(
		topicId: TopicId,
		author: VerifyingKey,
	): Promise<SimplifiedOperation<any>[]> {
		const logKey = `${topicId}/${author}`;
		const items = this.getItems();

		const operations = Object.entries(items).filter(([key, value]) =>
			key.startsWith(logKey),
		);

		return operations.map(([k, v]) => JSON.parse(v));
	}

	async getAuthorsForTopic(topicId: TopicId): Promise<VerifyingKey[]> {
		const logKey = `${topicId}/authors`;
		const items = this.getItems();

		const operations = Object.entries(items).filter(([key, value]) =>
			key.startsWith(logKey),
		);

		return operations.map(([k, v]) => v);
	}

	async create<T>(topicId: TopicId, body: T, timestamp?: number) {
		return this.createSync(topicId, body, timestamp);
	}

	/** Synchronous version of create, for writing data before the stores read it. */
	createSync<T>(
		topicId: TopicId,
		body: T,
		timestamp?: number,
		auth?: GroupsArgs,
	) {
		const logKey = `${topicId}/${this._myPubKey}`;
		const items = this.getItems();
		const operations: SimplifiedOperation<any>[] = Object.entries(items)
			.filter(([key]) => key.startsWith(logKey))
			.map(([, v]) => JSON.parse(v));
		const descendantOperations = operations.sort(
			(o1, o2) => o2.header.seq_num - o1.header.seq_num,
		);
		const lastOperation = descendantOperations[0];

		const header: SimplifiedHeader = {
			backlink: lastOperation?.hash,
			previous: [],
			verifying_key: this._myPubKey,
			seq_num: lastOperation ? lastOperation.header.seq_num + 1 : 0,
			timestamp: timestamp ?? Date.now(),
			topic_id: topicId,
			auth,
		};

		const headerHash = hash(header);

		const operation: SimplifiedOperation<T> = {
			body,
			hash: headerHash,
			header,
		};

		const storageKey = `${topicId}/${this._myPubKey}/${header.seq_num}`;
		localStorage.setItem(storageKey, JSON.stringify(operation));

		const authorsLogKey = `${topicId}/authors/${this._myPubKey}`;
		localStorage.setItem(authorsLogKey, this._myPubKey);

		operationsWritten.emit('new-operation', {
			topicId,
			author: this._myPubKey,
			operation,
		});

		return headerHash;
	}

	onNewOperation(
		handler: (topicId: TopicId, operation: SimplifiedOperation<any>) => void,
	): UnsubscribeFunction {
		return operationsWritten.on('new-operation', event => {
			handler(event.topicId, event.operation);
		});
	}
}

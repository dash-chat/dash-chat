import { blake2b, blake2bHex } from 'blakejs';
import Emittery, { type UnsubscribeFunction } from 'emittery';

import type { LogsClient } from './../p2panda/logs-client';
import type {
	SimplifiedHeader,
	SimplifiedOperation,
} from './../p2panda/simplified-types';
import type {
	Hash,
	Header,
	LogId,
	Operation,
	PublicKey,
	TopicId,
} from './../p2panda/types';

export function hash<T>(obj: T): Hash {
	return blake2bHex(JSON.stringify(obj));
}

export class LocalStorageLogsClient implements LogsClient<any> {
	emitter = new Emittery();

	constructor(protected _myPubKey: PublicKey) {}

	async myPubKey() {
		return this._myPubKey;
	}

	getItems() {
		const items = { ...localStorage };
		return items;
	}

	async getLog(
		topicId: TopicId,
		author: PublicKey,
	): Promise<SimplifiedOperation<any>[]> {
		const logKey = `${topicId}/${author}`;
		const items = this.getItems();

		const operations = Object.entries(items).filter(([key, value]) =>
			key.startsWith(logKey),
		);

		return operations.map(([k, v]) => JSON.parse(v));
	}

	async getAuthorsForTopic(topicId: TopicId): Promise<PublicKey[]> {
		const logKey = `${topicId}/authors`;
		const items = this.getItems();

		const operations = Object.entries(items).filter(([key, value]) =>
			key.startsWith(logKey),
		);

		return operations.map(([k, v]) => v);
	}

	async create<T>(topicId: TopicId, body: T) {
		const log = await this.getLog(topicId, this._myPubKey);
		const descendantOperations = log.sort(
			(o1, o2) => o2.header.timestamp - o1.header.timestamp,
		);
		const lastOperation = descendantOperations[0];

		const header: SimplifiedHeader = {
			backlink: lastOperation?.hash,
			previous: [],
			public_key: this._myPubKey,
			seq_num: lastOperation ? lastOperation.header.seq_num + 1 : 0,
			timestamp: Date.now() * 1000,
			topic_id: topicId,
		};

		const headerHash = hash(header);

		const operation: SimplifiedOperation<T> = {
			body,
			hash: headerHash,
			header,
		};

		const logKey = `${topicId}/${this._myPubKey}/${header.seq_num}`;
		localStorage.setItem(logKey, JSON.stringify(operation));

		const authorsLogKey = `${topicId}/authors/${this._myPubKey}`;
		localStorage.setItem(authorsLogKey, this._myPubKey);

		this.emitter.emit('new-operation', {
			topicId,
			author: this._myPubKey,
			operation,
		});
	}

	onNewOperation(
		handler: (topicId: TopicId, operation: SimplifiedOperation<any>) => void,
	): UnsubscribeFunction {
		return this.emitter.on('new-operation', event => {
			handler(event.topicId, event.operation);
		});
	}
}

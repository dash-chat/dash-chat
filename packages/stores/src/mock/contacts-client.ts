import type {
	AddContactResult,
	IContactsClient,
	Profile,
} from '../contacts/contacts-client';
import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import type { Payload } from '../types';
import { type LocalStorageLogsClient, chatIdForDevices } from './client';

function profileOf(op: SimplifiedOperation<Payload>): Profile | undefined {
	const body = op.body;
	if (body?.type !== 'Chat' && body?.type !== 'DeviceGroupPayload')
		return undefined;
	return body.payload.type === 'SetProfile' ? body.payload.payload : undefined;
}

export class MockContactsClient implements IContactsClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private agentId: AgentId,
		private deviceId: DeviceId,
		private deviceGroupTopicId: TopicId,
		private inboxTopics: TopicId[],
		private knownDevices: Record<AgentId, DeviceId>,
	) {}

	async myAgentId(): Promise<AgentId> {
		return this.agentId;
	}

	async myDeviceId(): Promise<DeviceId> {
		return this.deviceId;
	}

	async agentForDevice(_devicePubkey: DeviceId): Promise<AgentId | undefined> {
		return undefined;
	}

	onAgentsIntroduced(): () => void {
		return () => {};
	}

	onProfileChanged(handler: (agentId: AgentId | undefined) => void) {
		return this.logsClient.emitter.on(
			'new-operation',
			({ operation }: { operation: SimplifiedOperation<Payload> }) => {
				if (profileOf(operation) !== undefined) handler(undefined);
			},
		);
	}

	async directChatId(devicePubkey: DeviceId): Promise<string> {
		return chatIdForDevices(this.deviceId, devicePubkey);
	}

	async setProfile(profile: Profile): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'SetProfile', payload: profile },
		});
	}

	/** My profile from my device group; a contact's from our direct chat. */
	async getProfile(agentId: AgentId): Promise<Profile | undefined> {
		const peerDevice = this.knownDevices[agentId];
		const [topicId, author] =
			agentId === this.agentId
				? [this.deviceGroupTopicId, this.deviceId]
				: [chatIdForDevices(this.deviceId, peerDevice), peerDevice];
		if (author === undefined) return undefined;
		const log: SimplifiedOperation<Payload>[] = await this.logsClient.getLog(
			topicId,
			author,
		);
		const newest = log
			.filter(op => profileOf(op) !== undefined)
			.sort((a, b) => b.header.timestamp - a.header.timestamp)[0];
		return newest === undefined ? undefined : profileOf(newest);
	}

	async createContactCode(): Promise<string> {
		const bytes = new Uint8Array(45);
		crypto.getRandomValues(bytes);
		return btoa(String.fromCharCode(...Array.from(bytes)));
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(_contactCode: string): Promise<AddContactResult> {
		return {
			kind: 'NewRequest',
			chatId: chatIdForDevices(this.deviceId, this.deviceId),
		};
	}

	async acceptContact(agentId: AgentId): Promise<void> {
		const peerDevice = this.knownDevices[agentId];
		if (peerDevice === undefined)
			throw new Error(`Unknown demo device for agent ${agentId}`);
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: {
				type: 'AddContact',
				payload: {
					agent_id: agentId,
					direct_chat_topic_id: chatIdForDevices(this.deviceId, peerDevice),
				},
			},
		});
	}

	async blockContact(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'BlockAgent', payload: agentId },
		});
	}

	async unblockContact(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'UnblockAgent', payload: agentId },
		});
	}

	async reportContact(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: {
				type: 'ReportContact',
				payload: { agent_id: agentId, device_ids: [], mailbox_ids: [] },
			},
		});
	}
}

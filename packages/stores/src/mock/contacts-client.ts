import type { IContactsClient, Profile } from '../contacts/contacts-client';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import { type LocalStorageLogsClient, chatIdForDevices } from './client';

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

	async directChatId(devicePubkey: DeviceId): Promise<string> {
		return chatIdForDevices(this.deviceId, devicePubkey);
	}

	async setProfile(profile: Profile): Promise<void> {
		await this.logsClient.create(personalTopicFor(this.agentId), {
			type: 'Announcements',
			payload: { type: 'SetProfile', payload: profile },
		});
	}

	async getProfile(_agentId: AgentId): Promise<Profile | undefined> {
		return undefined;
	}

	async createContactCode(): Promise<string> {
		const bytes = new Uint8Array(45);
		crypto.getRandomValues(bytes);
		return btoa(String.fromCharCode(...Array.from(bytes)));
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(_contactCode: string): Promise<string> {
		return chatIdForDevices(this.deviceId, this.deviceId);
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

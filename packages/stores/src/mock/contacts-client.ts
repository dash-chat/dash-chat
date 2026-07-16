import type { IContactsClient, Profile } from '../contacts/contacts-client';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import type { LocalStorageLogsClient } from './client';

export class MockContactsClient implements IContactsClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private agentId: AgentId,
		private deviceId: DeviceId,
		private deviceGroupTopicId: TopicId,
		private inboxTopics: TopicId[],
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

	async setProfile(profile: Profile): Promise<void> {
		await this.logsClient.create(personalTopicFor(this.agentId), {
			type: 'Announcements',
			payload: { type: 'SetProfile', payload: profile },
		});
	}

	async createContactCode(): Promise<string> {
		return Math.random().toString(36).slice(2);
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(_contactCode: string): Promise<DeviceId> {
		return this.deviceId;
	}

	async acceptContact(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'AddContact', payload: { agent_id: agentId } },
		});
	}

	async rejectContactRequest(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'RejectContactRequest', payload: agentId },
		});
	}
}

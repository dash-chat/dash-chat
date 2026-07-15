import type { IContactsClient, Profile } from '../contacts/contacts-client';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import type { ContactCode, InboxTopic } from '../types';
import { ShareIntent } from '../types';
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

	async createContactCode(): Promise<ContactCode> {
		return {
			device_pubkey: this.deviceId,
			inbox_topic: this.inboxTopics[0]
				? { topic: this.inboxTopics[0], expires_at: Date.now() + 86400000 }
				: undefined,
			share_intent: ShareIntent.AddContact,
		};
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(_contactCode: ContactCode): Promise<void> {}

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
}

import { LogsClient, waitForOperation } from '../p2panda/logs-client';
import { AgentId, DeviceId, type TopicId } from '../p2panda/types';
import { ContactCode, Payload } from '../types';
import { invoke } from '../utils/invoke';

export interface Profile {
	name: string;
	surname: string | undefined;
	avatar: string | undefined;
	about: string | undefined;
}

export function fullName(profile: Profile): string {
	return `${profile.name}${profile.surname ? ` ${profile.surname}` : ''}`;
}

export interface IContactsClient {
	/// Profiles

	myAgentId(): Promise<AgentId>;

	myDeviceId(): Promise<DeviceId>;

	// Sets the profile for this user
	setProfile(profile: Profile): Promise<void>;

	/// contacts

	// Creates a new contact code to be shared
	createContactCode(): Promise<ContactCode>;

	activeInboxTopics(): Promise<TopicId[]>;

	// getContacts(): Promise<Array<VerifyingKey>>;

	// Add contact
	addContact(code: ContactCode): Promise<void>;

	// Reject contact request
	rejectContactRequest(agentId: AgentId): Promise<void>;

	// Remove contact
	// removeContact(contact: ContactId): Promise<void>;

	/// Contact Requests

	// // Send contact request to the given user
	// sendContactRequest(userId: UserId): Promise<void>;

	// // Accept contact request for the given user
	// acceptContactRequest(userId: UserId): Promise<void>;

	// // Reject contact request for the given user
	// rejectContactRequest(userId: UserId): Promise<void>;

	// // Cancel contact request for the given user
	// cancelContactRequest(userId: UserId): Promise<void>;
}

export class ContactsClient implements IContactsClient {
	constructor(protected logsClient: LogsClient<Payload>) {}

	myAgentId(): Promise<AgentId> {
		return invoke('my_agent_id');
	}

	myDeviceId(): Promise<DeviceId> {
		return invoke('my_device_id');
	}

	async setProfile(profile: Profile): Promise<void> {
		return invoke('set_profile', {
			profile,
		});
	}

	createContactCode(): Promise<ContactCode> {
		return invoke('create_contact_code');
	}

	activeInboxTopics(): Promise<TopicId[]> {
		return invoke('active_inbox_topics');
	}

	async addContact(contactCode: ContactCode): Promise<void> {
		await Promise.all([
			invoke('add_contact', { contactCode }),
			waitForOperation(
				this.logsClient,
				op =>
					op.body?.payload.type === 'AddContact' &&
					op.body.payload.payload.agent_id === contactCode.agent_id,
			),
		]);
	}

	async rejectContactRequest(agentId: AgentId): Promise<void> {
		await Promise.all([
			invoke('reject_contact_request', { agentId }),
			waitForOperation(
				this.logsClient,
				op =>
					op.body?.payload.type === 'RejectContactRequest' &&
					op.body.payload.payload === agentId,
			),
		]);
	}

	// getContacts(): Promise<Array<VerifyingKey>> {
	// 	return invoke('get_contacts');
	// }

	// removeContact(contactId: ContactId): Promise<void> {
	// 	return invoke('remove_contact', {
	// 		contactId,
	// 	});
	// }
}

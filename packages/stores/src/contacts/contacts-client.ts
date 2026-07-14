import { LogsClient, waitForOperation } from '../p2panda/logs-client';
import { AgentId, DeviceId, type TopicId } from '../p2panda/types';
import { ContactCode, Payload } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

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

	// Resolve the agent id recorded for a device pubkey, if the contact is
	// established. Undefined while an outgoing request is still pending.
	agentForDevice(devicePubkey: DeviceId): Promise<AgentId | undefined>;

	// Sets the profile for this user
	setProfile(profile: Profile): Promise<void>;

	/// contacts

	// Creates a new contact code to be shared
	createContactCode(): Promise<ContactCode>;

	activeInboxTopics(): Promise<TopicId[]>;

	// getContacts(): Promise<Array<VerifyingKey>>;

	// Add contact
	addContact(code: ContactCode): Promise<void>;

	// Accept an incoming contact request
	acceptContact(agentId: AgentId): Promise<void>;

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
		return invokeAfterSetup('my_agent_id');
	}

	myDeviceId(): Promise<DeviceId> {
		return invokeAfterSetup('my_device_id');
	}

	async agentForDevice(devicePubkey: DeviceId): Promise<AgentId | undefined> {
		return (
			(await invokeAfterSetup<AgentId | null>('agent_for_device', {
				devicePubkey,
			})) ?? undefined
		);
	}

	async setProfile(profile: Profile): Promise<void> {
		return invokeAfterSetup('set_profile', {
			profile,
		});
	}

	createContactCode(): Promise<ContactCode> {
		return invokeAfterSetup('create_contact_code');
	}

	activeInboxTopics(): Promise<TopicId[]> {
		return invokeAfterSetup('active_inbox_topics');
	}

	async addContact(contactCode: ContactCode): Promise<void> {
		await invokeAfterSetup('add_contact', { contactCode });
	}

	async acceptContact(agentId: AgentId): Promise<void> {
		await Promise.all([
			invokeAfterSetup('accept_contact', { agentId }),
			waitForOperation(
				this.logsClient,
				op =>
					op.body?.payload.type === 'AddContact' &&
					op.body.payload.payload.agent_id === agentId,
			),
		]);
	}

	async rejectContactRequest(agentId: AgentId): Promise<void> {
		await Promise.all([
			invokeAfterSetup('reject_contact_request', { agentId }),
			waitForOperation(
				this.logsClient,
				op =>
					op.body?.payload.type === 'RejectContactRequest' &&
					op.body.payload.payload === agentId,
			),
		]);
	}

	// getContacts(): Promise<Array<VerifyingKey>> {
	// 	return invokeAfterSetup('get_contacts');
	// }

	// removeContact(contactId: ContactId): Promise<void> {
	// 	return invokeAfterSetup('remove_contact', {
	// 		contactId,
	// 	});
	// }
}

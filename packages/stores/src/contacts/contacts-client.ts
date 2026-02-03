import { invoke } from '@tauri-apps/api/core';

import { AgentId, type TopicId } from '../p2panda/types';
import { ContactCode } from '../types';

export interface Profile {
	name: string;
	avatar: string | undefined;
}

export interface IContactsClient {
	/// Profiles

	myAgentId(): Promise<AgentId>;

	// Sets the profile for this user
	setProfile(profile: Profile): Promise<void>;

	/// contacts

	// Creates a new contact code to be shared (reuses existing if valid)
	getOrCreateContactCode(): Promise<ContactCode>;

	// Resets the contact code, invalidating the old one
	resetContactCode(): Promise<ContactCode>;

	activeInboxTopics(): Promise<TopicId[]>

	// getContacts(): Promise<Array<PublicKey>>;

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
	myAgentId(): Promise<AgentId> {
		return invoke('my_agent_id');
	}

	async setProfile(profile: Profile): Promise<void> {
		return invoke('set_profile', {
			profile,
		});
	}

	getOrCreateContactCode(): Promise<ContactCode> {
		return invoke('get_or_create_contact_code');
	}

	resetContactCode(): Promise<ContactCode> {
		return invoke('reset_contact_code');
	}

	activeInboxTopics(): Promise<TopicId[]> {
		return invoke('active_inbox_topics');
	}

	addContact(contactCode: ContactCode): Promise<void> {
		return invoke('add_contact', {
			contactCode,
		});
	}

	rejectContactRequest(agentId: AgentId): Promise<void> {
		return invoke('reject_contact_request', {
			agentId,
		});
	}

	// getContacts(): Promise<Array<PublicKey>> {
	// 	return invoke('get_contacts');
	// }

	// removeContact(contactId: ContactId): Promise<void> {
	// 	return invoke('remove_contact', {
	// 		contactId,
	// 	});
	// }
}

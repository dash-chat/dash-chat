import type { Message, MessagesStore } from 'dash-chat-stores';

/** Reactive edit state shared by the direct- and group-chat pages: holds the
 * message being edited and the composer draft, and submits edits to the store. */
export class MessageEditing {
	editing = $state<Message | undefined>(undefined);
	value = $state('');

	constructor(private store: MessagesStore) {}

	start(message: Message) {
		this.editing = message;
		this.value = message.content.message;
	}

	cancel() {
		this.editing = undefined;
		this.value = '';
	}

	submit = async (message: Message, text: string): Promise<void> => {
		await this.store.editMessage(message, text);
	};
}

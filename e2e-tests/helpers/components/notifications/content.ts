/** A notification as the app posts it. */
export interface NotificationContent {
	/** For a chat message, the sender's name. */
	title: string;
	/** For a chat message, the latest one. */
	body: string;
	/** The group a chat message was posted in, null outside a group chat, or
	 * undefined where the platform does not expose it apart from the title. */
	conversation?: string | null;
}

/** One notification the OS is holding, as its content shows it. */
export interface DeliveredNotification extends NotificationContent {
	/** Every string it shows, title and body included. */
	texts: string[];
}

/** `content` as a failure prints it. */
export function describeContent(content: NotificationContent): string {
	const title =
		content.conversation === null || content.conversation === undefined
			? content.title
			: `${content.conversation} › ${content.title}`;
	return `${title} | ${content.body}`;
}

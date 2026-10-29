import { type Hash, type Message, hasBody } from 'dash-chat-stores';

/** The text a search looks at: a message's current text, which is also its
 * caption when it carries media. */
function searchableText(message: Message): string {
	return hasBody(message.content) ? message.content.message : '';
}

/** The hashes of the messages whose text contains `query`, ignoring case,
 * oldest first. */
export function searchMessages<T extends { kind: string; message?: Message }>(
	days: Array<{ eventsGroups: Array<Array<[Hash, T]>> }>,
	query: string,
): Hash[] {
	if (query === '') return [];
	const needle = query.toLowerCase();
	return days
		.flatMap(day => day.eventsGroups.flat())
		.filter(
			([, event]) =>
				event.message !== undefined &&
				searchableText(event.message).toLowerCase().includes(needle),
		)
		.map(([hash]) => hash);
}

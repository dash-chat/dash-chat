import {
	type DeliveredNotification,
	type NotificationContent,
	describeContent,
} from './content';
import type { NotificationHelper } from './notification-helper';

interface ToHaveDeliveredOptions {
	/** How many notifications must show `expected`. Without it, at least one. */
	times?: number;
}

function showsAll(
	notification: DeliveredNotification,
	expected: Partial<NotificationContent>,
): boolean {
	return (
		(expected.title === undefined || notification.title === expected.title) &&
		(expected.body === undefined || notification.body === expected.body) &&
		(expected.conversation === undefined ||
			notification.conversation === expected.conversation)
	);
}

export const notificationMatchers = {
	/** Wait until the device holds a notification showing every field of
	 *  `expected` (as many as `times` says), or with `.not`, none. */
	async toHaveDelivered(
		this: { isNot?: boolean },
		notifications: NotificationHelper,
		expected: Partial<NotificationContent>,
		{ times }: ToHaveDeliveredOptions = {},
	) {
		const isNot = this.isNot === true;
		const holds = (shown: DeliveredNotification[]) => {
			const count = shown.filter(n => showsAll(n, expected)).length;
			return times === undefined ? count > 0 : count === times;
		};
		const shown = await notifications.waitForDelivered(
			all => holds(all) !== isNot,
		);
		const wanted = describeContent({
			title: expected.title ?? '*',
			body: expected.body ?? '*',
			conversation: expected.conversation,
		});
		const held =
			shown.length === 0
				? 'nothing'
				: shown.map(n => `"${describeContent(n)}"`).join(', ');
		return {
			pass: holds(shown),
			message: () =>
				`expected the device ${isNot ? 'not ' : ''}to show "${wanted}"` +
				`${times === undefined ? '' : ` ${times} times`}; it holds ${held}`,
		};
	},
};

declare global {
	namespace ExpectWebdriverIO {
		interface Matchers<R extends void | Promise<void>, T> {
			toHaveDelivered(
				expected: Partial<NotificationContent>,
				options?: ToHaveDeliveredOptions,
			): Promise<void>;
		}
	}
}

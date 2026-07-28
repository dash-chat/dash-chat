import { createSubscriber } from 'svelte/reactivity';

export const lessThanAMinuteAgo = (timestamp: number) =>
	Date.now() - timestamp < 60 * 1000;
export const moreThanAnHourAgo = (timestamp: number) =>
	Date.now() - timestamp > 60 * 60 * 1000;
export const moreThanAWeekAgo = (timestamp: number) =>
	Date.now() - timestamp > 7 * 24 * 60 * 60 * 1000;
export const moreThanAYearAgo = (timestamp: number) =>
	Date.now() - timestamp > 365 * 24 * 60 * 60 * 1000;
export const inToday = (timestamp: number) =>
	timestamp >= todayFirstTimestamp();

const todayFirstTimestamp = () => {
	const today = new Date();
	today.setHours(0);
	today.setMinutes(0);
	today.setSeconds(0);
	today.setMilliseconds(0);
	return today.valueOf();
};

const yesterdayFirstTimestamp = () => {
	const yesterday = new Date();
	yesterday.setDate(new Date().getDate() - 1);
	yesterday.setHours(0);
	yesterday.setMinutes(0);
	yesterday.setSeconds(0);
	yesterday.setMilliseconds(0);
	return yesterday.valueOf();
};

export const beforeYesterday = (timestamp: number) =>
	timestamp < yesterdayFirstTimestamp();

export const inYesterday = (timestamp: number) =>
	yesterdayFirstTimestamp() <= timestamp && timestamp < todayFirstTimestamp();

export const sleep = (ms: number) =>
	new Promise(resolve => setTimeout(() => resolve(undefined), ms));

/** One subscriber per distinct expiry, shared by every reader of that instant
 * so a single timer serves them all. */
const windowSubscribers = new Map<number, () => void>();

function subscriberFor(deadline: number): () => void {
	let subscribe = windowSubscribers.get(deadline);
	if (!subscribe) {
		subscribe = createSubscriber(update => {
			// setTimeout overflows past 2^31-1 ms and would fire immediately,
			// spinning on a timestamp set far in the future by a skewed clock.
			const delay = Math.min(deadline - Date.now(), 2 ** 31 - 1);
			const timer = setTimeout(() => {
				windowSubscribers.delete(deadline);
				update();
			}, delay);
			return () => {
				clearTimeout(timer);
				windowSubscribers.delete(deadline);
			};
		});
		windowSubscribers.set(deadline, subscribe);
	}
	return subscribe;
}

/** Whether `windowMs` has yet to elapse since `timestamp`.
 *
 * Reactive, unlike the predicates above: read inside a `$derived` or `$effect`,
 * it re-runs that computation the moment the window closes. Use it wherever the
 * reader outlives the window — a component that mounts long before it is shown
 * would otherwise evaluate the window once and stay frozen. Timestamps already
 * past their window subscribe to nothing, so they cost no timer. */
export function withinWindow(timestamp: number, windowMs: number): boolean {
	const deadline = timestamp + windowMs;
	if (deadline <= Date.now()) return false;
	subscriberFor(deadline)();
	return true;
}

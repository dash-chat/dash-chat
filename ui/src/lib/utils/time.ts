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

/** Format a duration in milliseconds as `m:ss` (e.g. a voice-note length). */
export const formatDuration = (ms: number): string => {
	const totalSeconds = Math.floor(ms / 1000);
	const minutes = Math.floor(totalSeconds / 60);
	const seconds = totalSeconds % 60;
	return `${minutes}:${seconds.toString().padStart(2, '0')}`;
};

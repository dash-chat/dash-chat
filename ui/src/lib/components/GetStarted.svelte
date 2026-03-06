<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiAccountMultiplePlus,
		mdiAccountPlus,
		mdiCamera,
		mdiClose,
		mdiPalette,
	} from '@mdi/js';
	import { useTheme } from 'konsta/svelte';

	interface Card {
		id: string;
		label: () => string;
		icon: string;
		href: string;
		color: string;
		hidden?: boolean;
	}

	const DISMISSED_KEY = 'get-started-dismissed';

	let { hasAvatar = false, visible = $bindable(true) }: { hasAvatar?: boolean; visible?: boolean } =
		$props();

	const theme = $derived(useTheme());

	const allCards: Card[] = [
		{
			id: 'add-contact',
			label: () => m.addContact(),
			icon: mdiAccountPlus,
			href: '/new-message/add-contact',
			color: 'bg-amber-100 dark:bg-amber-900/30',
		},
		{
			id: 'add-photo',
			label: () => m.addPhoto(),
			icon: mdiCamera,
			href: '/settings/profile/edit-photo',
			color: 'bg-blue-100 dark:bg-blue-900/30',
		},
		{
			id: 'chat-color',
			label: () => m.chatColor(),
			icon: mdiPalette,
			href: '/settings/appearance',
			color: 'bg-purple-100 dark:bg-purple-900/30',
		},
		{
			id: 'new-group',
			label: () => m.newGroup(),
			icon: mdiAccountMultiplePlus,
			href: '/new-group',
			color: 'bg-green-100 dark:bg-green-900/30',
			hidden: true,
		},
	];

	const glassClasses =
		'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass';

	function getDismissed(): string[] {
		try {
			return JSON.parse(localStorage.getItem(DISMISSED_KEY) || '[]');
		} catch {
			return [];
		}
	}

	let dismissed = $state(getDismissed());

	let visibleCards = $derived(
		allCards.filter((c) => {
			if (c.hidden) return false;
			if (dismissed.includes(c.id)) return false;
			if (c.id === 'add-photo' && hasAvatar) return false;
			return true;
		}),
	);
	$effect(() => {
		visible = visibleCards.length > 0;
	});

	function dismiss(id: string) {
		dismissed = [...dismissed, id];
		try {
			localStorage.setItem(DISMISSED_KEY, JSON.stringify(dismissed));
		} catch (e) {
			console.error('Failed to persist dismissed cards:', e);
		}
	}
</script>

{#if visibleCards.length > 0}
	<div class="px-4 pb-4">
		<p class="mb-2 text-base font-medium">{m.getStarted()}</p>
		<div class="flex gap-3 overflow-x-auto">
			{#each visibleCards as card}
				<a
					href={card.href}
					data-testid="get-started-{card.id}"
					class="relative flex w-44 flex-col items-center rounded-2xl px-6 py-5 {theme === 'ios'
						? glassClasses
						: card.color}"
				>
					<button
						class="absolute right-2 top-2 p-1 opacity-40"
						onclick={(e) => {
							e.preventDefault();
							dismiss(card.id);
						}}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 18px"></wa-icon>
					</button>
					<wa-icon src={wrapPathInSvg(card.icon)} style="font-size: 28px; opacity: 0.6">
					</wa-icon>
					<span class="mt-2 text-center text-sm">{card.label()}</span>
				</a>
			{/each}
		</div>
	</div>
{/if}

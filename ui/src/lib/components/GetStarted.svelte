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

	const allCards: Card[] = [
		{
			id: 'add-contact',
			label: () => m.addContact(),
			icon: mdiAccountPlus,
			href: '/new-message/add-contact',
			color: 'bg-amber-50 dark:bg-amber-950/30',
		},
		{
			id: 'add-photo',
			label: () => m.addPhoto(),
			icon: mdiCamera,
			href: '/settings/profile/edit-photo',
			color: 'bg-sky-50 dark:bg-sky-950/30',
		},
		{
			id: 'chat-color',
			label: () => m.chatColor(),
			icon: mdiPalette,
			href: '/settings/appearance',
			color: 'bg-violet-50 dark:bg-violet-950/30',
		},
		{
			id: 'new-group',
			label: () => m.newGroup(),
			icon: mdiAccountMultiplePlus,
			href: '/new-group',
			color: 'bg-emerald-50 dark:bg-emerald-950/30',
			hidden: true,
		},
	];

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
		<p class="mb-2 text-base font-semibold">{m.getStarted()}</p>
		<div class="flex gap-3 overflow-x-auto pb-1">
			{#each visibleCards as card}
				<a
					href={card.href}
					data-testid="get-started-{card.id}"
					class="relative flex w-40 shrink-0 flex-col items-center rounded-xl border border-black/5 px-5 pb-5 pt-8 dark:border-white/10 {card.color}"
				>
					<button
						class="absolute right-1.5 top-1.5 p-1 text-black/30 dark:text-white/30"
						onclick={(e) => {
							e.preventDefault();
							dismiss(card.id);
						}}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 16px"></wa-icon>
					</button>
					<wa-icon src={wrapPathInSvg(card.icon)} style="font-size: 24px; opacity: 0.7">
					</wa-icon>
					<span class="mt-2 text-center text-sm font-medium">{card.label()}</span>
				</a>
			{/each}
		</div>
	</div>
{/if}

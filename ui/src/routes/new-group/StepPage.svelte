<script lang="ts">
	import { Page, Navbar, NavbarBackLink, Button, Link } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		onBack?: () => void;
		backTestId?: string;
		actionLabel: string;
		onAction: () => void;
		actionLinkTestId?: string;
		actionBtnTestId?: string;
		children: Snippet;
	}

	let {
		title,
		onBack,
		backTestId,
		actionLabel,
		onAction,
		actionLinkTestId,
		actionBtnTestId,
		children,
	}: Props = $props();
</script>

<Page>
	<Navbar {title} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink
				onClick={onBack ?? (() => window.history.back())}
				data-testid={backTestId}
			/>
		{/snippet}

		{#snippet right()}
			{#if isIos}
				<Link onClick={onAction} data-testid={actionLinkTestId}>
					{actionLabel}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	{@render children()}

	{#if !isIos}
		<Button
			onClick={onAction}
			data-testid={actionBtnTestId}
			class="fixed-action-btn"
			rounded
		>
			{actionLabel}
		</Button>
	{/if}
</Page>

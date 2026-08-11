<script lang="ts">
	import { Navbar, NavbarBackLink, Page } from 'konsta/svelte';
	import { Marked } from 'marked';
	import { m } from '$lib/paraglide/messages.js';
	import { getLocale } from '$lib/paraglide/runtime';

	let { onBack }: { onBack: () => void } = $props();

	const agreements = import.meta.glob<string>('$messages/eula/*.md', {
		query: '?raw',
		import: 'default',
	});

	const load =
		agreements[`/messages/eula/${getLocale()}.md`] ??
		agreements['/messages/eula/en.md'];

	const markdown = new Marked().use({
		breaks: true,
		renderer: { link: ({ text }) => text },
	});

	const agreement = load().then(source => markdown.parse(source));
</script>

<Page>
	<Navbar
		title={m.termsAndPrivacyPolicy()}
		titleClass="opacity1"
		transparent={true}
	>
		{#snippet left()}
			<NavbarBackLink onClick={onBack} data-testid="eula-back" />
		{/snippet}
	</Navbar>

	<div class="eula" data-testid="eula-body">
		{#await agreement then html}
			{@html html}
		{/await}
	</div>
</Page>

<style>
	.eula {
		max-width: 38rem;
		margin-inline: auto;
		padding: 1.5rem 1rem 3rem;
		font-size: 15px;
		line-height: 1.625;
	}
	/* Konsta sets the theme font on an ancestor; without this the agreement's
	   plain elements pick up the webawesome theme's font instead. */
	.eula :global(:is(h1, h2, p, ul, li, strong)) {
		font-family: inherit;
	}
	.eula :global(h1) {
		margin-bottom: 0.25rem;
		font-size: 1.5rem;
		font-weight: 700;
	}
	.eula :global(h2) {
		margin-block: 2rem 0.5rem;
		font-size: 1.125rem;
		font-weight: 600;
	}
	.eula :global(p) {
		margin-bottom: 0.75rem;
	}
	.eula :global(ul) {
		margin-bottom: 0.75rem;
		list-style: disc;
		padding-inline-start: 1.25rem;
	}
	.eula :global(li) {
		margin-bottom: 0.25rem;
	}
	.eula :global(h1 + p) {
		margin-bottom: 2rem;
		font-size: 13px;
		opacity: 0.6;
	}
</style>

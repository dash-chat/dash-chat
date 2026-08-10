<script lang="ts">
	import {
		Actions,
		ActionsGroup,
		Dialog,
		DialogButton,
		Preloader,
	} from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from './ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
	import { showToast } from '$lib/utils/toasts';
	import type { Snippet } from 'svelte';

	type ActionResult = { success: true } | { success: false; error: string };

	type Props = {
		opened: boolean;
		onCancel: () => void;
		onConfirm: () => Promise<ActionResult>;
		title: string;
		children: Snippet;
		cancelText?: string;
		confirmText: string;
		confirmTestId?: string;
	};

	let {
		opened,
		onCancel,
		onConfirm,
		title,
		children,
		cancelText = m.cancel(),
		confirmText,
		confirmTestId,
	}: Props = $props();

	let loading = $state(false);

	async function handleConfirm() {
		loading = true;
		try {
			const result = await onConfirm();
			if (!result.success) {
				showToast(result.error, 'error');
			}
		} finally {
			loading = false;
		}
	}
</script>

{#if isIos}
	<Actions {opened} onBackdropClick={onCancel}>
		<ActionsGroup class="flex flex-col gap-3 p-2.5">
			<div class="flex flex-col gap-1 px-3.5 py-2 text-start">
				<span class="text-xl text-black dark:text-white">{title}</span>
				<span class="text-black/60 dark:text-white/60">
					{@render children()}
				</span>
			</div>
			<ActionButton
				destructive
				onClick={handleConfirm}
				disabled={loading}
				data-testid={confirmTestId}
			>
				{confirmText}
				{#if loading}
					<Preloader class="ms-2 h-4 w-4" />
				{/if}
			</ActionButton>
			<ActionButton onClick={onCancel} disabled={loading}>
				{cancelText}
			</ActionButton>
		</ActionsGroup>
	</Actions>
{:else}
	<Dialog {opened} onBackdropClick={onCancel} {title}>
		{@render children()}
		{#snippet buttons()}
			<DialogButton onClick={onCancel} disabled={loading}>
				{cancelText}
			</DialogButton>
			<DialogButton
				strong
				onClick={handleConfirm}
				disabled={loading}
				data-testid={confirmTestId}
			>
				{confirmText}
				{#if loading}
					<Preloader class="ms-2 h-4 w-4" />
				{/if}
			</DialogButton>
		{/snippet}
	</Dialog>
{/if}

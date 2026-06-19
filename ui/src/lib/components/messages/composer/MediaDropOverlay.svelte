<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { isTauriEnv } from '$lib/utils/environment';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { listen } from '@tauri-apps/api/event';

	interface Props {
		onFiles: (files: File[]) => void;
	}

	let { onFiles }: Props = $props();

	// HTML5 pipeline (browser dev mode and synthetic test events). Native
	// drops in Tauri are intercepted by wry and delivered through
	// onDragDropEvent instead, so trusted events are skipped here to avoid
	// double handling.
	let dragDepth = $state(0);
	let tauriDragging = $state(false);

	const dragging = $derived(dragDepth > 0 || tauriDragging);

	function skipHtml5(event: DragEvent): boolean {
		return (
			(isTauriEnv() && event.isTrusted) ||
			!Array.from(event.dataTransfer?.types ?? []).includes('Files')
		);
	}

	function onDragEnter(event: DragEvent) {
		if (skipHtml5(event)) return;
		event.preventDefault();
		dragDepth++;
	}

	function onDragOver(event: DragEvent) {
		if (skipHtml5(event)) return;
		event.preventDefault();
	}

	function onDragLeave(event: DragEvent) {
		if (skipHtml5(event)) return;
		dragDepth = Math.max(0, dragDepth - 1);
	}

	function onDrop(event: DragEvent) {
		if (skipHtml5(event)) return;
		event.preventDefault();
		dragDepth = 0;
		const files = Array.from(event.dataTransfer?.files ?? []);
		if (files.length > 0) onFiles(files);
	}

	const MIME_BY_EXTENSION: Record<string, string> = {
		png: 'image/png',
		jpg: 'image/jpeg',
		jpeg: 'image/jpeg',
		gif: 'image/gif',
		webp: 'image/webp',
		avif: 'image/avif',
		bmp: 'image/bmp',
		heic: 'image/heic',
		svg: 'image/svg+xml',
		mp4: 'video/mp4',
		mov: 'video/quicktime',
		webm: 'video/webm',
		pdf: 'application/pdf',
		txt: 'text/plain',
	};

	function mimeForName(name: string): string {
		const ext = name.split('.').pop()?.toLowerCase() ?? '';
		return MIME_BY_EXTENSION[ext] ?? 'application/octet-stream';
	}

	interface DroppedFile {
		name: string;
		data: number[];
	}

	// Native drops: the webview only tracks the overlay state from the
	// drag-drop event; the dropped files' bytes arrive from the Rust side
	// (`media_drop.rs`), which reads the paths so the webview needs no
	// filesystem read capability.
	$effect(() => {
		if (!isTauriEnv()) return;
		const unlisteners: (() => void)[] = [];
		let cancelled = false;
		const register = (u: () => void) => {
			if (cancelled) u();
			else unlisteners.push(u);
		};
		getCurrentWebview()
			.onDragDropEvent(event => {
				if (event.payload.type === 'enter' || event.payload.type === 'over') {
					tauriDragging = true;
				} else {
					tauriDragging = false;
				}
			})
			.then(register);
		listen<DroppedFile[]>('media://files-dropped', event => {
			const files = event.payload.map(
				f =>
					new File([new Uint8Array(f.data)], f.name, {
						type: mimeForName(f.name),
					}),
			);
			if (files.length > 0) onFiles(files);
		}).then(register);
		return () => {
			cancelled = true;
			unlisteners.forEach(u => u());
		};
	});
</script>

<svelte:window
	ondragenter={onDragEnter}
	ondragover={onDragOver}
	ondragleave={onDragLeave}
	ondrop={onDrop}
/>

{#if dragging}
	<div class="drop-overlay" data-testid="media-drop-overlay">
		<div class="drop-card">
			{m.dropFilesToSend()}
		</div>
	</div>
{/if}

<style>
	.drop-overlay {
		position: fixed;
		inset: 0;
		z-index: 30;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: center;
		justify-content: center;
		/* Never intercept the drag — events are handled on the window. */
		pointer-events: none;
	}

	.drop-card {
		border: 2px dashed white;
		border-radius: 12px;
		padding: 24px 32px;
		color: white;
		font-size: 16px;
		font-weight: 500;
		background: rgba(0, 0, 0, 0.35);
	}
</style>

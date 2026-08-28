<script lang="ts">
	import { mdiCheckCircle } from '@mdi/js';

	const uid = $props.id();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
	<defs>
		<!-- Punch the area around the front glyph out of the back one, so the
		     overlap works on any bubble background color. -->
		<mask id="cut-{uid}">
			<rect width="24" height="24" fill="white" />
			<circle cx="15.5" cy="12" r="8.5" fill="black" />
		</mask>
	</defs>
	<g mask="url(#cut-{uid})">
		<path
			d={mdiCheckCircle}
			fill="currentColor"
			transform="translate(0.2 4.2) scale(0.65)"
		/>
	</g>
	<path
		d={mdiCheckCircle}
		fill="currentColor"
		transform="translate(7.1 3.6) scale(0.7)"
	/>
</svg>

<style>
	svg {
		width: 100%;
		height: 100%;
	}
</style>

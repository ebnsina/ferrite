<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Segment {
		label: string;
		value: number;
		color: string; // Tailwind text-color class
	}

	interface Props {
		segments: Segment[];
		size?: number;
		thickness?: number;
		center?: Snippet;
	}

	let { segments, size = 160, thickness = 16, center }: Props = $props();

	const r = $derived((size - thickness) / 2);
	const c = $derived(2 * Math.PI * r);
	const total = $derived(Math.max(0, segments.reduce((s, x) => s + x.value, 0)));

	// Cumulative start fraction for each segment.
	const arcs = $derived.by(() => {
		let acc = 0;
		return segments.map((s) => {
			const frac = total > 0 ? s.value / total : 0;
			const arc = { ...s, frac, start: acc };
			acc += frac;
			return arc;
		});
	});
</script>

<div class="relative flex items-center justify-center" style={`width:${size}px;height:${size}px`}>
	<svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} class="relative">
		<!-- track -->
		<circle
			cx={size / 2}
			cy={size / 2}
			{r}
			fill="none"
			stroke="currentColor"
			stroke-width={thickness}
			class="text-surface-2"
		/>
		{#each arcs as a (a.label)}
			{#if a.frac > 0}
				<circle
					cx={size / 2}
					cy={size / 2}
					{r}
					fill="none"
					stroke="currentColor"
					stroke-width={thickness}
					stroke-linecap="round"
					stroke-dasharray={`${a.frac * c} ${c}`}
					stroke-dashoffset={-a.start * c}
					transform={`rotate(-90 ${size / 2} ${size / 2})`}
					class={a.color}
					style="transition: stroke-dasharray 0.5s ease"
				/>
			{/if}
		{/each}
	</svg>
	{#if center}
		<div class="absolute flex flex-col items-center justify-center text-center">
			{@render center()}
		</div>
	{/if}
</div>

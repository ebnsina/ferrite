<script lang="ts">
	// Labeled usage meter: value against an optional cap, with a filled bar.
	interface Props {
		label: string;
		value: string; // formatted display value
		fraction: number; // 0–1 fill (clamped)
		color?: string; // Tailwind text-color class
		hint?: string;
	}

	let { label, value, fraction, color = 'text-accent', hint }: Props = $props();
	const pct = $derived(Math.max(0, Math.min(1, fraction)) * 100);
</script>

<div>
	<div class="flex items-baseline justify-between">
		<span class="text-xs text-muted">{label}</span>
		<span class="mono text-sm font-semibold">{value}</span>
	</div>
	<div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-surface-2">
		<div
			class={`h-full rounded-full bg-current ${color}`}
			style={`width:${pct}%; transition: width 0.5s ease`}
		></div>
	</div>
	{#if hint}<p class="mt-1 text-[10px] text-muted">{hint}</p>{/if}
</div>

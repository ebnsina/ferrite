<script lang="ts">
	interface Props {
		/** 0..1 */
		value: number;
	}

	let { value }: Props = $props();

	const pct = $derived(Math.round(Math.min(1, Math.max(0, value)) * 100));
</script>

<div class="flex items-center gap-3">
	<div class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
		<div class="h-full rounded-full bg-accent transition-all duration-500" style={`width:${pct}%`}></div>
	</div>
	<span class="mono w-10 text-right text-xs text-muted">{pct}%</span>
</div>

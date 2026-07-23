<script lang="ts">
	import type { Asset } from '$lib/api/types';
	import { Icon } from '$lib/ui';
	import { FilmStrip, Spinner } from 'phosphor-svelte';

	interface Props {
		asset: Asset;
		class?: string;
	}
	let { asset, class: klass = 'h-12 w-20' }: Props = $props();

	let hovering = $state(false);
	// The thumbnail poster defaults to ~1s in; add a time hint for a nicer frame.
	const poster = $derived(asset.thumbnail_url ? `${asset.thumbnail_url}&time=1&width=240` : null);
</script>

<div
	class={`relative shrink-0 overflow-hidden rounded-md border border-border bg-surface-2 ${klass}`}
	onmouseenter={() => (hovering = true)}
	onmouseleave={() => (hovering = false)}
	role="img"
	aria-label={asset.filename}
>
	{#if asset.status === 'processing'}
		<div class="flex h-full w-full items-center justify-center text-muted">
			<Icon icon={Spinner} size={16} class="animate-spin" />
		</div>
	{:else if poster}
		<img src={poster} alt="" class="h-full w-full object-cover" loading="lazy" />
		{#if hovering && asset.preview_url}
			<!-- svelte-ignore a11y_media_has_caption -->
			<video
				src={asset.preview_url}
				class="absolute inset-0 h-full w-full object-cover"
				autoplay
				muted
				loop
				playsinline
			></video>
		{/if}
	{:else}
		<div class="flex h-full w-full items-center justify-center text-muted">
			<Icon icon={FilmStrip} size={16} />
		</div>
	{/if}
</div>

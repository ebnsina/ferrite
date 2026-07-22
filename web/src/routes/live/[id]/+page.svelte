<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Card, Icon } from '$lib/ui';
	import { getLiveStream } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { LiveStream } from '$lib/api/types';
	import { ArrowLeft01Icon, Copy01Icon, Tick01Icon } from '@hugeicons/core-free-icons';

	const id = $derived(page.params.id!);
	let stream = $state<LiveStream | null>(null);
	let error = $state<string | null>(null);
	let copied = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	async function load() {
		try {
			stream = await getLiveStream(id);
			error = null;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load stream.';
		}
	}

	function copy(label: string, value: string) {
		navigator.clipboard.writeText(value);
		copied = label;
		setTimeout(() => (copied = null), 1500);
	}

	// Play the live stream via SRS HTTP-FLV (low latency, reliable) using mpegts.js.
	function livePlayer(node: HTMLVideoElement, flvUrl: string) {
		let player: { destroy(): void } | undefined;
		(async () => {
			await import('$lib/vendor/mpegts.js'); // UMD side effect: sets window.mpegts
			const mpegts = window.mpegts;
			if (!mpegts?.isSupported()) return;
			const p = mpegts.createPlayer(
				{ type: 'flv', isLive: true, url: flvUrl },
				{ enableWorker: false, liveBufferLatencyChasing: true }
			);
			p.attachMediaElement(node);
			p.load();
			p.play();
			player = p;
		})();
		return {
			destroy: () => {
				try {
					player?.destroy();
				} catch {
					/* ignore teardown races */
				}
			}
		};
	}

	onMount(() => {
		load();
		timer = setInterval(load, 4000);
	});
	onDestroy(() => clearInterval(timer));
</script>

<div class="mx-auto max-w-3xl">
	<a href="/live" class="mb-6 inline-flex items-center gap-1 text-sm text-muted hover:text-fg">
		<Icon icon={ArrowLeft01Icon} size={16} /> Live
	</a>

	{#if error}
		<div class="rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{:else if !stream}
		<p class="text-sm text-muted">Loading…</p>
	{:else}
		<div class="mb-4 flex items-center justify-between">
			<h1 class="text-xl font-semibold tracking-tight">{stream.name}</h1>
			{#if stream.live}
				<span class="mono rounded-full border border-danger/30 bg-danger/10 px-2 py-0.5 text-xs font-medium text-danger">LIVE</span>
			{:else}
				<span class="mono rounded-full border border-border bg-surface-2 px-2 py-0.5 text-xs text-muted">offline</span>
			{/if}
		</div>

		<Card class="mb-6 overflow-hidden !p-0">
			{#if stream.live}
				<!-- No {#key}: flv_url is constant, so the player mounts once and
				     persists across status polls instead of churning. -->
				<!-- svelte-ignore a11y_media_has_caption -->
				<video use:livePlayer={stream.flv_url} controls autoplay muted playsinline class="aspect-video w-full bg-black"></video>
			{:else}
				<div class="flex aspect-video w-full items-center justify-center bg-black text-sm text-muted">
					Waiting for the stream to start…
				</div>
			{/if}
		</Card>

		<Card>
			<h2 class="mb-3 text-sm font-medium text-muted">Ingest (OBS / ffmpeg)</h2>
			{#each [{ label: 'RTMP URL', value: stream.ingest_url }, { label: 'Stream key', value: stream.stream_key }, { label: 'SRT URL', value: stream.srt_url }] as row (row.label)}
				<div class="mb-2 flex items-center gap-2">
					<span class="w-24 shrink-0 text-xs text-muted">{row.label}</span>
					<code class="mono flex-1 truncate rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs">{row.value}</code>
					<button onclick={() => copy(row.label, row.value)} class="text-muted hover:text-fg" aria-label="Copy">
						{#if copied === row.label}<Icon icon={Tick01Icon} size={16} />{:else}<Icon icon={Copy01Icon} size={16} />{/if}
					</button>
				</div>
			{/each}
		</Card>
	{/if}
</div>

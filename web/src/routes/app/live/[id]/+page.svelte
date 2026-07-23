<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Card, Icon, Button, Sheet, toast } from '$lib/ui';
	import {
		getLiveStream,
		listTargets,
		createTarget,
		deleteTarget,
		clipLive,
		type SimulcastTarget
	} from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { LiveStream } from '$lib/api/types';
	import {
		ArrowLeft01Icon,
		Copy01Icon,
		Tick01Icon,
		Scissor01Icon,
		Share08Icon,
		Delete02Icon
	} from '@hugeicons/core-free-icons';

	const id = $derived(page.params.id!);
	let stream = $state<LiveStream | null>(null);
	let targets = $state<SimulcastTarget[]>([]);
	let error = $state<string | null>(null);
	let copied = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';

	async function load() {
		try {
			stream = await getLiveStream(id);
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load this stream.');
		}
	}

	async function loadTargets() {
		try {
			targets = await listTargets(id);
		} catch {
			// non-fatal
		}
	}

	// Simulcast add sheet
	let tOpen = $state(false);
	let tName = $state('');
	let tUrl = $state('');
	let tKey = $state('');
	let tBusy = $state(false);
	let tErr = $state<string | null>(null);

	function openTarget() {
		tName = '';
		tUrl = '';
		tKey = '';
		tErr = null;
		tOpen = true;
	}

	async function addTarget() {
		if (!tName.trim() || !tUrl.trim() || !tKey.trim()) {
			tErr = 'All fields are required.';
			return;
		}
		tBusy = true;
		tErr = null;
		try {
			await createTarget(id, tName.trim(), tUrl.trim(), tKey.trim());
			tOpen = false;
			toast.success(`Now restreaming to ${tName.trim()}.`);
			await loadTargets();
		} catch (e) {
			tErr = humanizeError(e instanceof ApiError ? e.message : null, 'Could not add destination.');
		} finally {
			tBusy = false;
		}
	}

	async function removeTarget(tid: string) {
		try {
			await deleteTarget(id, tid);
			toast.success('Restream destination removed.');
			await loadTargets();
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not remove destination.'));
		}
	}

	// Live clip sheet
	let clipOpen = $state(false);
	let clipDuration = $state(15);
	let clipBusy = $state(false);
	let clipMsg = $state<string | null>(null);

	async function doClipLive() {
		clipBusy = true;
		clipMsg = null;
		try {
			await clipLive(id, clipDuration);
			clipMsg = 'Capturing… your clip will appear in Assets shortly.';
			toast.success('Capturing the last moments — your clip will appear in Videos shortly.');
		} catch (e) {
			clipMsg = humanizeError(e instanceof ApiError ? e.message : null, 'Could not start capture.');
		} finally {
			clipBusy = false;
		}
	}

	function copy(label: string, value: string) {
		navigator.clipboard.writeText(value);
		copied = label;
		toast.success('Copied to clipboard.');
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
		loadTargets();
		timer = setInterval(load, 4000);
	});
	onDestroy(() => clearInterval(timer));
</script>

<div class="mx-auto max-w-3xl">
	<a href="/app/live" class="mb-6 inline-flex items-center gap-1 text-sm text-muted hover:text-fg">
		<Icon icon={ArrowLeft01Icon} size={16} /> Live
	</a>

	{#if error}
		<div class="rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{:else if !stream}
		<p class="text-sm text-muted">Loading…</p>
	{:else}
		<div class="mb-4 flex items-center justify-between gap-3">
			<h1 class="text-xl font-semibold tracking-tight">{stream.name}</h1>
			<div class="flex items-center gap-2">
				{#if stream.live}
					<Button size="sm" variant="secondary" onclick={() => ((clipMsg = null), (clipOpen = true))}>
						<Icon icon={Scissor01Icon} size={15} /> Clip live
					</Button>
					<span
						class="mono rounded-full border border-danger/30 bg-danger/10 px-2 py-0.5 text-xs font-medium text-danger"
						>LIVE</span
					>
				{:else}
					<span
						class="mono rounded-full border border-border bg-surface-2 px-2 py-0.5 text-xs text-muted"
						>offline</span
					>
				{/if}
			</div>
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

		<Card class="mt-6">
			<div class="mb-3 flex items-center justify-between">
				<h2 class="flex items-center gap-2 text-sm font-medium text-muted">
					<Icon icon={Share08Icon} size={16} /> Simulcast
				</h2>
				<Button size="sm" variant="secondary" onclick={openTarget}>Add destination</Button>
			</div>
			<p class="mb-4 text-xs text-muted">
				Restream this broadcast to YouTube, Twitch, or any RTMP destination while you're live.
			</p>
			{#if targets.length === 0}
				<p class="py-4 text-center text-sm text-muted">No destinations yet.</p>
			{:else}
				<div class="divide-y divide-border">
					{#each targets as t (t.id)}
						<div class="flex items-center justify-between gap-3 py-2.5">
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">{t.name}</p>
								<p class="mono truncate text-xs text-muted">{t.url}</p>
							</div>
							<button
								onclick={() => removeTarget(t.id)}
								aria-label="Remove destination"
								class="shrink-0 rounded-lg p-1.5 text-muted transition-colors hover:bg-danger/10 hover:text-danger"
							>
								<Icon icon={Delete02Icon} size={16} />
							</button>
						</div>
					{/each}
				</div>
			{/if}
		</Card>
	{/if}
</div>

<Sheet open={tOpen} onclose={() => (tOpen = false)} title="Add simulcast destination"
	description="Ferrite will relay your live stream here while you broadcast.">
	<div class="flex flex-col gap-4">
		<div>
			<label for="t-name" class="mb-1.5 block text-xs font-medium text-muted">Name</label>
			<input id="t-name" bind:value={tName} placeholder="YouTube" class={inputCls} />
		</div>
		<div>
			<label for="t-url" class="mb-1.5 block text-xs font-medium text-muted">RTMP URL</label>
			<input id="t-url" bind:value={tUrl} placeholder="rtmp://a.rtmp.youtube.com/live2" class={inputCls} />
		</div>
		<div>
			<label for="t-key" class="mb-1.5 block text-xs font-medium text-muted">Stream key</label>
			<input id="t-key" bind:value={tKey} placeholder="xxxx-xxxx-xxxx" class={inputCls} />
		</div>
		{#if tErr}<p class="text-sm text-danger">{tErr}</p>{/if}
	</div>
	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (tOpen = false)}>Cancel</Button>
			<Button disabled={tBusy} onclick={addTarget}>{tBusy ? 'Adding…' : 'Add destination'}</Button>
		</div>
	{/snippet}
</Sheet>

<Sheet open={clipOpen} onclose={() => (clipOpen = false)} title="Clip the live stream"
	description="Capture the next few seconds into a new asset.">
	<div class="flex flex-col gap-4">
		<div>
			<label for="clip-dur" class="mb-1.5 block text-xs font-medium text-muted"
				>Duration (seconds)</label
			>
			<input id="clip-dur" type="number" min="1" max="120" bind:value={clipDuration} class={inputCls} />
		</div>
		{#if clipMsg}<p class="text-sm text-accent">{clipMsg}</p>{/if}
	</div>
	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (clipOpen = false)}>Close</Button>
			<Button disabled={clipBusy} onclick={doClipLive}>{clipBusy ? 'Capturing…' : 'Capture'}</Button>
		</div>
	{/snippet}
</Sheet>

<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, Button, Icon } from '$lib/ui';
	import { listLiveStreams, createLiveStream, getLiveStream } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { LiveStream } from '$lib/api/types';
	import { timeAgo } from '$lib/format';
	import { LiveStreaming01Icon, DotIcon, PlusSignIcon } from '@hugeicons/core-free-icons';

	let streams = $state<LiveStream[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let name = $state('');
	let creating = $state(false);
	let timer: ReturnType<typeof setInterval>;

	async function load() {
		try {
			streams = await listLiveStreams();
			error = null;
			// Refresh live status per stream (list endpoint doesn't poll the ingest server).
			await Promise.all(
				streams.map(async (s, i) => {
					streams[i] = await getLiveStream(s.id);
				})
			);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load streams.';
		} finally {
			loading = false;
		}
	}

	async function create() {
		if (!name.trim()) return;
		creating = true;
		error = null;
		try {
			await createLiveStream(name.trim());
			name = '';
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not create stream.';
		} finally {
			creating = false;
		}
	}

	onMount(() => {
		load();
		timer = setInterval(load, 5000);
	});
	onDestroy(() => clearInterval(timer));
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Live</h1>
		<p class="mt-1 text-sm text-muted">Create a stream, then push RTMP from OBS or ffmpeg.</p>
	</div>

	<Card class="mb-6">
		<div class="flex gap-2">
			<input
				bind:value={name}
				placeholder="Stream name"
				class="flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				onkeydown={(e) => e.key === 'Enter' && create()}
			/>
			<Button disabled={creating || !name.trim()} onclick={create}>
				<Icon icon={PlusSignIcon} size={16} /> New stream
			</Button>
		</div>
	</Card>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<Card>
		{#if loading}
			<p class="py-8 text-center text-sm text-muted">Loading…</p>
		{:else if streams.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><Icon icon={LiveStreaming01Icon} size={32} /></span>
				<p class="font-medium">No live streams</p>
				<p class="mt-1 text-sm text-muted">Create one to get an RTMP ingest URL.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each streams as s (s.id)}
					<a href={`/app/live/${s.id}`} class="flex items-center gap-4 py-3 transition-colors hover:bg-surface-2">
						<span class={s.live ? 'text-danger' : 'text-muted'}>
							<Icon icon={DotIcon} size={22} />
						</span>
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm font-medium">{s.name}</p>
							<p class="mono text-xs text-muted">{timeAgo(s.created_at)}</p>
						</div>
						{#if s.live}
							<span class="mono rounded-full border border-danger/30 bg-danger/10 px-2 py-0.5 text-xs font-medium text-danger">LIVE</span>
						{:else}
							<span class="mono rounded-full border border-border bg-surface-2 px-2 py-0.5 text-xs text-muted">offline</span>
						{/if}
					</a>
				{/each}
			</div>
		{/if}
	</Card>
</div>

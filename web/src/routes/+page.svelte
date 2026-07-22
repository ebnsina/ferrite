<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, StatusPill, ProgressBar } from '$lib/ui';
	import { listAssets, listJobs } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Asset, Job, JobState } from '$lib/api/types';
	import { bytes, timeAgo } from '$lib/format';
	import { Upload, Film, ListVideo, HardDrive } from '@lucide/svelte';

	let assets = $state<Asset[]>([]);
	let jobs = $state<Job[]>([]);
	let error = $state<string | null>(null);

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];
	const activeCount = $derived(jobs.filter((j) => ACTIVE.includes(j.state)).length);
	const totalBytes = $derived(assets.reduce((s, a) => s + (a.bytes ?? 0), 0));
	const recent = $derived(jobs.slice(0, 5));

	onMount(async () => {
		try {
			[assets, jobs] = await Promise.all([listAssets(), listJobs()]);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load dashboard.';
		}
	});

	const stats = $derived([
		{ label: 'Assets', value: String(assets.length), icon: Film },
		{ label: 'Active jobs', value: String(activeCount), icon: ListVideo },
		{ label: 'Storage', value: bytes(totalBytes), icon: HardDrive }
	]);
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Dashboard</h1>
			<p class="mt-1 text-sm text-muted">Transcode overview for your workspace.</p>
		</div>
		<a href="/assets"><Button><Upload size={16} /> Upload video</Button></a>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<div class="mb-8 grid gap-4 sm:grid-cols-3">
		{#each stats as stat (stat.label)}
			<Card>
				<div class="flex items-center justify-between">
					<div>
						<p class="text-sm text-muted">{stat.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{stat.value}</p>
					</div>
					<span class="text-muted"><stat.icon size={22} /></span>
				</div>
			</Card>
		{/each}
	</div>

	<Card>
		<h2 class="mb-4 text-sm font-medium text-muted">Recent jobs</h2>
		{#if recent.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><ListVideo size={32} /></span>
				<p class="font-medium">No jobs yet</p>
				<p class="mt-1 text-sm text-muted">Upload a video to kick off your first transcode.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each recent as job (job.id)}
					<div class="flex items-center gap-4 py-3">
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<div class="min-w-0 flex-1">
							{#if ACTIVE.includes(job.state)}
								<ProgressBar value={job.progress} />
							{:else}
								<p class="text-xs text-muted">{timeAgo(job.queued_at)}</p>
							{/if}
						</div>
						<StatusPill state={job.state} />
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>

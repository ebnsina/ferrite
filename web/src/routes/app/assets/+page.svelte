<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { Card, Button, Icon, Sheet } from '$lib/ui';
	import {
		listAssets,
		createAsset,
		uploadToPresigned,
		completeAsset,
		createJob,
		createJobsBatch,
		clipAsset
	} from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { clipSchema, validate } from '$lib/schemas';
	import AssetThumb from '$lib/components/AssetThumb.svelte';
	import type { Asset } from '$lib/api/types';
	import { bytes, timeAgo } from '$lib/format';
	import {
		Upload01Icon,
		Film01Icon,
		Loading03Icon,
		PlayIcon,
		CloudUploadIcon,
		Scissor01Icon
	} from '@hugeicons/core-free-icons';

	let assets = $state<Asset[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Upload sheet
	let uploadOpen = $state(false);
	let uploading = $state(false);
	let uploadErr = $state<string | null>(null);
	let file = $state<File | null>(null);
	let dragging = $state(false);

	async function load() {
		try {
			assets = await listAssets();
			error = null;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load assets.';
		} finally {
			loading = false;
		}
	}
	onMount(load);

	function openUpload() {
		file = null;
		uploadErr = null;
		uploadOpen = true;
	}

	function pick(e: Event) {
		file = (e.target as HTMLInputElement).files?.[0] ?? null;
		uploadErr = null;
	}

	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragging = false;
		const f = e.dataTransfer?.files?.[0];
		if (f) {
			file = f;
			uploadErr = null;
		}
	}

	async function upload() {
		if (!file) return (uploadErr = 'Choose a video file first.');
		if (!file.type.startsWith('video/')) return (uploadErr = 'That doesn’t look like a video file.');
		uploading = true;
		uploadErr = null;
		try {
			const { asset, upload_url } = await createAsset(file.name);
			await uploadToPresigned(upload_url, file);
			await completeAsset(asset.id, file.size);
			uploadOpen = false;
			await load();
		} catch (err) {
			uploadErr = err instanceof Error ? err.message : 'Upload failed.';
		} finally {
			uploading = false;
		}
	}

	// Clip sheet
	let clipOpen = $state(false);
	let clipSource = $state<Asset | null>(null);
	let clipStart = $state('0:00');
	let clipEnd = $state('0:10');
	let clipName = $state('');
	let clipping = $state(false);
	let clipErrors = $state<Record<string, string>>({});

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';

	// Accept "mm:ss", "h:mm:ss", or plain seconds → seconds.
	function parseTime(v: string): number {
		const t = v.trim();
		if (!t) return NaN;
		if (!t.includes(':')) return Number(t);
		return t
			.split(':')
			.map(Number)
			.reduce((acc, part) => acc * 60 + part, 0);
	}

	function openClip(a: Asset) {
		clipSource = a;
		clipStart = '0:00';
		clipEnd = '0:10';
		clipName = '';
		clipErrors = {};
		clipOpen = true;
	}

	async function doClip() {
		if (!clipSource) return;
		clipErrors = {};
		const start = parseTime(clipStart);
		const end = parseTime(clipEnd);
		const v = validate(clipSchema, { start, end, name: clipName || undefined });
		if (!v.ok) return (clipErrors = v.errors);
		clipping = true;
		try {
			await clipAsset(clipSource.id, v.data.start, v.data.end, v.data.name);
			clipOpen = false;
			await load();
		} catch (e) {
			clipErrors = { end: e instanceof ApiError ? e.message : 'Could not create clip.' };
		} finally {
			clipping = false;
		}
	}

	async function transcode(assetId: string) {
		try {
			await createJob(assetId);
			goto('/app/jobs');
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not start transcode.';
		}
	}

	const readyAssets = $derived(assets.filter((a) => a.status === 'ready'));

	async function transcodeAll() {
		try {
			await createJobsBatch(readyAssets.map((a) => a.id));
			goto('/app/jobs');
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not start transcodes.';
		}
	}
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Assets</h1>
			<p class="mt-1 text-sm text-muted">Upload source videos to transcode.</p>
		</div>
		<div class="flex gap-2">
			{#if readyAssets.length > 1}
				<Button variant="secondary" onclick={transcodeAll}>
					<Icon icon={PlayIcon} size={16} /> Transcode all ({readyAssets.length})
				</Button>
			{/if}
			<Button onclick={openUpload}><Icon icon={Upload01Icon} size={16} /> Upload video</Button>
		</div>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<Card>
		{#if loading}
			<p class="py-8 text-center text-sm text-muted">Loading…</p>
		{:else if assets.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><Icon icon={Film01Icon} size={32} /></span>
				<p class="font-medium">No assets yet</p>
				<p class="mt-1 text-sm text-muted">Upload your first video to get started.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each assets as a (a.id)}
					<div class="flex items-center gap-4 py-3">
						<AssetThumb asset={a} />
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm font-medium">{a.filename}</p>
							<p class="mono text-xs text-muted">{bytes(a.bytes)} · {timeAgo(a.created_at)}</p>
						</div>
						<span class="mono text-xs text-muted">{a.status}</span>
						{#if a.status === 'ready'}
							<Button size="sm" variant="ghost" onclick={() => openClip(a)}>
								<Icon icon={Scissor01Icon} size={15} /> Clip
							</Button>
							<Button size="sm" variant="secondary" onclick={() => transcode(a.id)}>Transcode</Button>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>

<Sheet
	open={uploadOpen}
	onclose={() => (uploadOpen = false)}
	title="Upload video"
	description="The file uploads directly to storage — it never passes through the API."
>
	<label
		ondragover={(e) => {
			e.preventDefault();
			dragging = true;
		}}
		ondragleave={() => (dragging = false)}
		ondrop={onDrop}
		class={`flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed px-6 py-12 text-center transition-colors ${
			dragging ? 'border-accent bg-accent-soft' : 'border-border hover:border-accent/50'
		}`}
	>
		<input type="file" accept="video/*" class="hidden" onchange={pick} />
		<span class="mb-3 text-accent"><Icon icon={CloudUploadIcon} size={36} /></span>
		{#if file}
			<p class="text-sm font-medium">{file.name}</p>
			<p class="mono mt-1 text-xs text-muted">{bytes(file.size)}</p>
			<p class="mt-2 text-xs text-muted">Click to choose a different file</p>
		{:else}
			<p class="text-sm font-medium">Drop a video here</p>
			<p class="mt-1 text-xs text-muted">or click to browse</p>
		{/if}
	</label>
	{#if uploadErr}<p class="mt-3 text-sm text-danger">{uploadErr}</p>{/if}

	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (uploadOpen = false)}>Cancel</Button>
			<Button disabled={uploading || !file} onclick={upload}>
				{#if uploading}<Icon icon={Loading03Icon} size={16} class="animate-spin" /> Uploading…{:else}Upload{/if}
			</Button>
		</div>
	{/snippet}
</Sheet>

<Sheet
	open={clipOpen}
	onclose={() => (clipOpen = false)}
	title="Create a clip"
	description={clipSource ? `Trim a section of ${clipSource.filename} into a new asset.` : ''}
>
	<div class="flex flex-col gap-4">
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="clip-start" class="mb-1.5 block text-xs font-medium text-muted">Start</label>
				<input id="clip-start" bind:value={clipStart} placeholder="0:00" class={inputCls} />
				{#if clipErrors.start}<p class="mt-1.5 text-sm text-danger">{clipErrors.start}</p>{/if}
			</div>
			<div>
				<label for="clip-end" class="mb-1.5 block text-xs font-medium text-muted">End</label>
				<input id="clip-end" bind:value={clipEnd} placeholder="0:10" class={inputCls} />
				{#if clipErrors.end}<p class="mt-1.5 text-sm text-danger">{clipErrors.end}</p>{/if}
			</div>
		</div>
		<p class="text-xs text-muted">Use mm:ss (e.g. 1:30) or seconds (e.g. 90).</p>
		<div>
			<label for="clip-name" class="mb-1.5 block text-xs font-medium text-muted"
				>Name <span class="text-muted/60">(optional)</span></label
			>
			<input id="clip-name" bind:value={clipName} placeholder="highlight.mp4" class={inputCls} />
		</div>
	</div>

	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (clipOpen = false)}>Cancel</Button>
			<Button disabled={clipping} onclick={doClip}>{clipping ? 'Clipping…' : 'Create clip'}</Button>
		</div>
	{/snippet}
</Sheet>

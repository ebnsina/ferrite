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
		clipAsset,
		makeShorts,
		getProvenance,
		getModeration,
		type Provenance,
		type Moderation
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
		Scissor01Icon,
		AiVideoIcon,
		ShieldIcon,
		CheckmarkCircle02Icon,
		Cancel01Icon
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

	// Provenance sheet
	let provOpen = $state(false);
	let provAsset = $state<Asset | null>(null);
	let prov = $state<Provenance | null>(null);
	let mod = $state<Moderation | null>(null);
	let provLoading = $state(false);
	let provNone = $state(false);

	async function openProv(a: Asset) {
		provAsset = a;
		prov = null;
		mod = null;
		provNone = false;
		provLoading = true;
		provOpen = true;
		const [p, m] = await Promise.allSettled([getProvenance(a.id), getModeration(a.id)]);
		if (p.status === 'fulfilled') prov = p.value;
		else provNone = true;
		if (m.status === 'fulfilled') mod = m.value;
		provLoading = false;
	}

	// AI shorts sheet
	let shortsOpen = $state(false);
	let shortsAsset = $state<Asset | null>(null);
	let shortsCount = $state(3);
	let shortsBusy = $state(false);
	let shortsErr = $state<string | null>(null);

	function openShorts(a: Asset) {
		shortsAsset = a;
		shortsCount = 3;
		shortsErr = null;
		shortsOpen = true;
	}

	async function doShorts() {
		if (!shortsAsset) return;
		shortsBusy = true;
		shortsErr = null;
		try {
			await makeShorts(shortsAsset.id, shortsCount);
			shortsOpen = false;
			goto('/app/jobs');
		} catch (e) {
			shortsErr = e instanceof ApiError ? e.message : 'Could not start.';
			shortsBusy = false;
		}
	}

	// Transcode options sheet
	let txOpen = $state(false);
	let txAsset = $state<Asset | null>(null);
	let txMp4 = $state(false);
	let txAudio = $state(false);
	let txCaptions = $state(false);
	let txEncrypt = $state(false);
	let txWatermark = $state(false);
	let txPosition = $state<'tl' | 'tr' | 'bl' | 'br'>('br');
	let txBusy = $state(false);

	function openTranscode(a: Asset) {
		txAsset = a;
		txMp4 = false;
		txAudio = false;
		txCaptions = false;
		txEncrypt = false;
		txWatermark = false;
		txPosition = 'br';
		txOpen = true;
	}

	async function startTranscode() {
		if (!txAsset) return;
		txBusy = true;
		try {
			await createJob(txAsset.id, {
				mp4: txMp4,
				audio: txAudio,
				captions: txCaptions,
				encrypt: txEncrypt,
				watermark: txWatermark ? { position: txPosition, opacity: 0.85 } : undefined
			});
			goto('/app/jobs');
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not start transcode.';
			txBusy = false;
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
							<button
								onclick={() => openProv(a)}
								aria-label="Content credentials"
								title="Content credentials"
								class="rounded-lg p-1.5 text-muted transition-colors hover:bg-surface-2 hover:text-accent"
							>
								<Icon icon={ShieldIcon} size={16} />
							</button>
							<Button size="sm" variant="ghost" onclick={() => openShorts(a)}>
								<Icon icon={AiVideoIcon} size={15} /> Shorts
							</Button>
							<Button size="sm" variant="ghost" onclick={() => openClip(a)}>
								<Icon icon={Scissor01Icon} size={15} /> Clip
							</Button>
							<Button size="sm" variant="secondary" onclick={() => openTranscode(a)}>Transcode</Button>
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

<Sheet
	open={txOpen}
	onclose={() => (txOpen = false)}
	title="Transcode options"
	description={txAsset ? `Adaptive HLS + DASH is always produced for ${txAsset.filename}.` : ''}
>
	<div class="flex flex-col gap-4">
		<label class="flex items-start gap-3">
			<input type="checkbox" bind:checked={txMp4} class="mt-0.5 accent-accent" />
			<span>
				<span class="block text-sm font-medium">MP4 download</span>
				<span class="block text-xs text-muted">A progressive, downloadable MP4 (up to 1080p).</span>
			</span>
		</label>
		<label class="flex items-start gap-3">
			<input type="checkbox" bind:checked={txAudio} class="mt-0.5 accent-accent" />
			<span>
				<span class="block text-sm font-medium">Audio-only</span>
				<span class="block text-xs text-muted">Extract an M4A audio track (podcasts, transcripts).</span>
			</span>
		</label>
		<label class="flex items-start gap-3">
			<input type="checkbox" bind:checked={txCaptions} class="mt-0.5 accent-accent" />
			<span>
				<span class="block text-sm font-medium">Auto-captions</span>
				<span class="block text-xs text-muted">Transcribe speech to a WebVTT subtitle track.</span>
			</span>
		</label>
		<label class="flex items-start gap-3">
			<input type="checkbox" bind:checked={txEncrypt} class="mt-0.5 accent-accent" />
			<span>
				<span class="block text-sm font-medium">Encrypt (AES-128)</span>
				<span class="block text-xs text-muted">Encrypt the HLS stream (no DASH output).</span>
			</span>
		</label>
		<label class="flex items-start gap-3">
			<input type="checkbox" bind:checked={txWatermark} class="mt-0.5 accent-accent" />
			<span>
				<span class="block text-sm font-medium">Watermark</span>
				<span class="block text-xs text-muted"
					>Overlay your brand logo on the stream + MP4 (set it in Settings).</span
				>
			</span>
		</label>
		{#if txWatermark}
			<div class="ml-7">
				<label for="tx-pos" class="mb-1.5 block text-xs font-medium text-muted">Position</label>
				<select id="tx-pos" bind:value={txPosition} class={inputCls}>
					<option value="br">Bottom-right</option>
					<option value="bl">Bottom-left</option>
					<option value="tr">Top-right</option>
					<option value="tl">Top-left</option>
				</select>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (txOpen = false)}>Cancel</Button>
			<Button disabled={txBusy} onclick={startTranscode}>
				{txBusy ? 'Starting…' : 'Start transcode'}
			</Button>
		</div>
	{/snippet}
</Sheet>

<Sheet
	open={shortsOpen}
	onclose={() => (shortsOpen = false)}
	title="Generate AI shorts"
	description={shortsAsset
		? `Find highlights in ${shortsAsset.filename} and turn them into vertical clips.`
		: ''}
>
	<div class="flex flex-col gap-4">
		<div>
			<label for="shorts-count" class="mb-1.5 block text-xs font-medium text-muted"
				>Number of shorts</label
			>
			<input
				id="shorts-count"
				type="number"
				min="1"
				max="10"
				bind:value={shortsCount}
				class={inputCls}
			/>
		</div>
		<p class="rounded-lg border border-border bg-surface-2 p-3 text-xs text-muted">
			Ferrite transcribes the audio, picks the best moments, and reframes each to 9:16 with
			captions. Finished shorts appear here as new assets.
		</p>
		{#if shortsErr}<p class="text-sm text-danger">{shortsErr}</p>{/if}
	</div>

	{#snippet footer()}
		<div class="flex justify-end gap-2">
			<Button variant="secondary" onclick={() => (shortsOpen = false)}>Cancel</Button>
			<Button disabled={shortsBusy} onclick={doShorts}>
				{shortsBusy ? 'Starting…' : 'Generate'}
			</Button>
		</div>
	{/snippet}
</Sheet>

<Sheet
	open={provOpen}
	onclose={() => (provOpen = false)}
	title="Content credentials"
	description={provAsset ? provAsset.filename : ''}
>
	{#if provLoading}
		<p class="py-8 text-center text-sm text-muted">Verifying…</p>
	{:else if provNone || !prov}
		<div class="rounded-lg border border-border bg-surface-2 p-4 text-center">
			<span class="mb-2 inline-flex text-muted"><Icon icon={ShieldIcon} size={24} /></span>
			<p class="text-sm font-medium">No content credentials</p>
			<p class="mt-1 text-xs text-muted">
				Only Ferrite-produced assets (clips, shorts, live clips) are signed.
			</p>
		</div>
	{:else}
		<div
			class={`mb-4 flex items-center gap-3 rounded-lg border p-4 ${prov.verified ? 'border-success/30 bg-success/10' : 'border-danger/30 bg-danger/10'}`}
		>
			<span class={prov.verified ? 'text-success' : 'text-danger'}>
				<Icon icon={prov.verified ? CheckmarkCircle02Icon : Cancel01Icon} size={26} />
			</span>
			<div>
				<p class="text-sm font-semibold">
					{prov.verified ? 'Verified by Ferrite' : 'Verification failed'}
				</p>
				<p class="text-xs text-muted">
					Signature {prov.signature_valid ? 'valid' : 'invalid'} · Content
					{prov.content_matches ? 'unmodified' : 'modified'}
				</p>
			</div>
		</div>
		<dl class="flex flex-col gap-2 text-sm">
			{#each [['Tool', String(prov.manifest.tool ?? '—')], ['Operation', String(prov.manifest.operation ?? '—')], ['Created', String(prov.manifest.created_at ?? '—')], ['Algorithm', prov.algorithm]] as [k, v] (k)}
				<div class="flex justify-between gap-4 border-b border-border pb-2">
					<dt class="text-muted">{k}</dt>
					<dd class="truncate text-right font-medium">{v}</dd>
				</div>
			{/each}
			<div class="pt-1">
				<dt class="mb-1 text-xs text-muted">Content hash (SHA-256)</dt>
				<dd class="mono break-all text-xs">{String(prov.manifest.sha256 ?? '')}</dd>
			</div>
			<div class="pt-1">
				<dt class="mb-1 text-xs text-muted">Public key (Ed25519)</dt>
				<dd class="mono break-all text-xs text-muted">{prov.public_key}</dd>
			</div>
		</dl>
	{/if}

	{#if mod && mod.checked}
		<div class="mt-6">
			<h3 class="mb-2 text-sm font-medium text-muted">Content safety</h3>
			<div
				class={`flex items-center gap-3 rounded-lg border p-3 ${mod.flagged ? 'border-warning/40 bg-warning/10' : 'border-success/30 bg-success/10'}`}
			>
				<span class={mod.flagged ? 'text-warning' : 'text-success'}>
					<Icon icon={mod.flagged ? ShieldIcon : CheckmarkCircle02Icon} size={20} />
				</span>
				<div>
					<p class="text-sm font-medium">
						{mod.flagged ? 'Flagged for review' : 'No issues detected'}
					</p>
					{#if mod.categories.length}
						<p class="text-xs text-muted">{mod.categories.join(', ')}</p>
					{/if}
				</div>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<Button class="w-full" variant="secondary" onclick={() => (provOpen = false)}>Close</Button>
	{/snippet}
</Sheet>

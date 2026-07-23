<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { Icon } from '$lib/ui';
	import { dur } from '$lib/motion';
	import { MonitorPlay, CaretDown } from 'phosphor-svelte';

	interface Props {
		src: string; // HLS master playlist
		poster?: string;
		captionsUrl?: string;
		/** When set, playback analytics beacons are sent. */
		analytics?: { beaconUrl: string; job: string; token: string };
		class?: string;
	}
	let { src, poster, captionsUrl, analytics, class: klass = '' }: Props = $props();

	interface Level {
		i: number;
		label: string;
		bitrate: number;
	}
	let levels = $state<Level[]>([]);
	let selected = $state(-1);
	let active = $state(-1);
	let menuOpen = $state(false);
	let hls: import('hls.js').default | null = null;

	const currentLabel = $derived(
		selected >= 0
			? (levels.find((l) => l.i === selected)?.label ?? 'Auto')
			: active >= 0 && levels.find((l) => l.i === active)
				? `Auto · ${levels.find((l) => l.i === active)!.label}`
				: 'Auto'
	);

	function pick(i: number) {
		if (hls) {
			selected = i;
			hls.currentLevel = i;
		}
		menuOpen = false;
	}

	// --- analytics beacons ---
	const session =
		typeof crypto !== 'undefined' && crypto.randomUUID
			? crypto.randomUUID()
			: Math.random().toString(36).slice(2);
	let viewed = false;
	let timer: ReturnType<typeof setInterval> | undefined;

	function beacon(kind: 'view' | 'heartbeat' | 'ended', video: HTMLVideoElement) {
		if (!analytics) return;
		const body = JSON.stringify({
			job: analytics.job,
			token: analytics.token,
			session,
			kind,
			position: video.currentTime || 0,
			watched: kind === 'heartbeat' ? 10 : 0
		});
		navigator.sendBeacon?.(analytics.beaconUrl, new Blob([body], { type: 'application/json' }));
	}

	function player(node: HTMLVideoElement) {
		(async () => {
			const { default: Hls } = await import('hls.js');
			if (Hls.isSupported()) {
				hls = new Hls();
				hls.loadSource(src);
				hls.attachMedia(node);
				hls.on(Hls.Events.MANIFEST_PARSED, () => {
					levels = hls!.levels
						.map((l, i) => ({
							i,
							bitrate: l.bitrate,
							label: l.height ? `${l.height}p` : `${Math.round(l.bitrate / 1000)}k`
						}))
						.sort((a, b) => b.bitrate - a.bitrate);
					active = hls!.currentLevel;
				});
				hls.on(Hls.Events.LEVEL_SWITCHED, (_e, d) => (active = d.level));
			} else if (node.canPlayType('application/vnd.apple.mpegurl')) {
				node.src = src;
			}
		})();

		// analytics events
		const onPlay = () => {
			if (!viewed) {
				viewed = true;
				beacon('view', node);
			}
			timer ??= setInterval(() => {
				if (!node.paused && !node.ended) beacon('heartbeat', node);
			}, 10000);
		};
		const onEnded = () => beacon('ended', node);
		node.addEventListener('playing', onPlay);
		node.addEventListener('ended', onEnded);

		return {
			destroy() {
				if (timer) clearInterval(timer);
				node.removeEventListener('playing', onPlay);
				node.removeEventListener('ended', onEnded);
				hls?.destroy();
			}
		};
	}
</script>

<div class={`relative ${klass}`}>
	<!-- svelte-ignore a11y_media_has_caption -->
	<video
		use:player
		controls
		playsinline
		crossorigin="anonymous"
		{poster}
		class="h-full w-full bg-black"
	>
		{#if captionsUrl}
			<track kind="subtitles" srclang="en" label="Captions" src={captionsUrl} default />
		{/if}
	</video>

	{#if levels.length > 1}
		<div class="absolute right-2 bottom-14">
			<button
				onclick={() => (menuOpen = !menuOpen)}
				class="mono inline-flex items-center gap-1 rounded-md bg-black/70 px-2 py-1 text-xs text-white hover:bg-black/85"
			>
				<Icon icon={MonitorPlay} size={13} />
				{currentLabel}
				<Icon icon={CaretDown} size={11} />
			</button>
			{#if menuOpen}
				<div
					class="absolute right-0 bottom-full mb-1 min-w-28 overflow-hidden rounded-md bg-black/90 text-white"
					transition:fly={{ y: 6, duration: dur(140), easing: cubicOut }}
				>
					<button
						onclick={() => pick(-1)}
						class={`mono block w-full px-3 py-1.5 text-left text-xs hover:bg-white/10 ${selected === -1 ? 'text-accent' : ''}`}
						>Auto</button
					>
					{#each levels as l (l.i)}
						<button
							onclick={() => pick(l.i)}
							class={`mono block w-full px-3 py-1.5 text-left text-xs hover:bg-white/10 ${selected === l.i ? 'text-accent' : ''}`}
							>{l.label}</button
						>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

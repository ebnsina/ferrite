// The vendored mpegts.js UMD bundle assigns itself to `window.mpegts` when
// loaded in a browser ESM context. Import the file for its side effect, then
// read `window.mpegts`.
interface MpegtsPlayer {
	attachMediaElement(el: HTMLMediaElement): void;
	load(): void;
	play(): void;
	pause(): void;
	unload(): void;
	detachMediaElement(): void;
	destroy(): void;
	on(event: string, cb: (...args: unknown[]) => void): void;
}

interface MpegtsApi {
	isSupported(): boolean;
	getFeatureList(): { mseLivePlayback: boolean };
	createPlayer(
		source: { type: string; isLive?: boolean; url: string },
		config?: { enableWorker?: boolean; liveBufferLatencyChasing?: boolean; lazyLoad?: boolean }
	): MpegtsPlayer;
}

declare global {
	interface Window {
		mpegts: MpegtsApi;
	}
}

export {};

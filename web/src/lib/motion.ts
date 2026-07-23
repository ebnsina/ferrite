// Shared motion helpers. Everything routes through `dur()` so a single
// prefers-reduced-motion check disables animation app-wide.
import { browser } from '$app/environment';

export const reducedMotion =
	browser && (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false);

/** Duration in ms, collapsed to 0 under reduced-motion. */
export const dur = (ms: number): number => (reducedMotion ? 0 : ms);

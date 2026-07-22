import type { HandleClientError } from '@sveltejs/kit';

// Client-side error handler. Attaches a correlation id so a user can quote it
// when reporting, and keeps the user-facing message generic.
export const handleError: HandleClientError = ({ error, event }) => {
	const errorId = crypto.randomUUID();
	console.error(`[client:${errorId}] ${event.url.pathname}`, error);

	return {
		message: 'Something went wrong. Please try again.',
		errorId
	};
};

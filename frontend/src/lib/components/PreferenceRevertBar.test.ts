import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import PreferenceRevertBar from './PreferenceRevertBar.svelte';
import { PREFERENCE_REVERT_SECONDS } from '$lib/preferences/schema';

describe('PreferenceRevertBar', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
		cleanup();
	});

	it('shows the initial countdown value', () => {
		render(PreferenceRevertBar, { props: { onKeep: () => {}, onRevert: () => {} } });
		expect(screen.getByText(String(PREFERENCE_REVERT_SECONDS))).toBeInTheDocument();
	});

	it('auto-reverts when the countdown reaches zero without action', () => {
		const onRevert = vi.fn();
		const onKeep = vi.fn();
		render(PreferenceRevertBar, { props: { onKeep, onRevert } });

		vi.advanceTimersByTime(PREFERENCE_REVERT_SECONDS * 1000);

		expect(onRevert).toHaveBeenCalledTimes(1);
		expect(onKeep).not.toHaveBeenCalled();
	});

	it('does not revert before the countdown elapses', () => {
		const onRevert = vi.fn();
		render(PreferenceRevertBar, { props: { onKeep: () => {}, onRevert } });

		vi.advanceTimersByTime((PREFERENCE_REVERT_SECONDS - 1) * 1000);

		expect(onRevert).not.toHaveBeenCalled();
	});

	it('calls onKeep when Keep is clicked', async () => {
		const onKeep = vi.fn();
		const onRevert = vi.fn();
		render(PreferenceRevertBar, { props: { onKeep, onRevert } });

		await fireEvent.click(screen.getByText('Keep'));

		expect(onKeep).toHaveBeenCalledTimes(1);
	});

	it('calls onRevert when Revert now is clicked', async () => {
		const onRevert = vi.fn();
		render(PreferenceRevertBar, { props: { onKeep: () => {}, onRevert } });

		await fireEvent.click(screen.getByText('Revert now'));

		expect(onRevert).toHaveBeenCalledTimes(1);
	});
});

import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import PreferenceControl from './PreferenceControl.svelte';

const options = [
	{ value: 'auto', label: 'Auto' },
	{ value: 'large', label: 'Large' }
];

afterEach(() => cleanup());

describe('PreferenceControl', () => {
	it('renders the label, description, and all options', () => {
		render(PreferenceControl, {
			props: { label: 'Text size', description: 'Scales text', value: 'auto', options, onSelect: () => {} }
		});
		expect(screen.getByText('Text size')).toBeInTheDocument();
		expect(screen.getByText('Scales text')).toBeInTheDocument();
		expect(screen.getByRole('radio', { name: 'Auto' })).toBeInTheDocument();
		expect(screen.getByRole('radio', { name: 'Large' })).toBeInTheDocument();
	});

	it('marks the current value as checked', () => {
		render(PreferenceControl, {
			props: { label: 'Text size', description: '', value: 'large', options, onSelect: () => {} }
		});
		expect(screen.getByRole('radio', { name: 'Large' })).toHaveAttribute('aria-checked', 'true');
		expect(screen.getByRole('radio', { name: 'Auto' })).toHaveAttribute('aria-checked', 'false');
	});

	it('calls onSelect with the chosen value', async () => {
		const onSelect = vi.fn();
		render(PreferenceControl, {
			props: { label: 'Text size', description: '', value: 'auto', options, onSelect }
		});
		await fireEvent.click(screen.getByRole('radio', { name: 'Large' }));
		expect(onSelect).toHaveBeenCalledWith('large');
	});
});

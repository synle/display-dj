import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Dropdown from './Dropdown';

describe('Dropdown', () => {
  /** Applies shared and context classes while preserving native select behavior. */
  it('combines classes and reports the selected value', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <Dropdown
        aria-label='Mode'
        className='context-dropdown'
        defaultValue='auto'
        onChange={onChange}>
        <option value='auto'>Auto</option>
        <option value='overlay'>Overlay</option>
      </Dropdown>,
    );

    const dropdown = screen.getByRole('combobox', { name: 'Mode' }) as HTMLSelectElement;
    expect(dropdown.classList.contains('dropdown')).toBe(true);
    expect(dropdown.classList.contains('context-dropdown')).toBe(true);

    await user.selectOptions(dropdown, 'overlay');

    expect(dropdown.value).toBe('overlay');
    expect(onChange).toHaveBeenCalledOnce();
  });
});

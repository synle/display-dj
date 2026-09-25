import type { ComponentProps } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import VolumeControl from './VolumeControl';

const outputState = {
  devices: [
    {
      id: 'speakers',
      name: 'Desk Speakers',
      originalName: 'MacBook Pro Speakers',
      state: 'enabled' as const,
      isBuiltIn: true,
    },
    {
      id: 'headphones',
      name: 'Headphones',
      originalName: 'USB Headphones',
      state: 'enabled' as const,
      isBuiltIn: false,
    },
  ],
  selectedDeviceId: 'speakers',
};

/** Renders the volume control with complete default props. */
function renderVolumeControl(overrides: Partial<ComponentProps<typeof VolumeControl>> = {}) {
  const props: ComponentProps<typeof VolumeControl> = {
    value: 50,
    onChange: vi.fn(),
    outputState,
    expanded: false,
    onToggleExpanded: vi.fn(),
    updatingDeviceId: null,
    onSelectOutput: vi.fn(),
    onRenameOutput: vi.fn(),
    onMoveOutput: vi.fn(),
    ...overrides,
  };
  return { ...render(<VolumeControl {...props} />), props };
}

describe('VolumeControl', () => {
  it('renders a slider with the given volume value', () => {
    renderVolumeControl({ value: 75 });
    const slider = screen.getByRole('slider');
    expect(slider).toHaveValue('75');
  });

  it('shows muted icon when volume is 0', () => {
    renderVolumeControl({ value: 0 });
    expect(screen.getByText('\uD83D\uDD07')).toBeInTheDocument();
  });

  it('shows speaker icon when volume is above 0', () => {
    renderVolumeControl();
    expect(screen.getByText('\uD83D\uDD0A')).toBeInTheDocument();
  });

  it('mutes (calls onChange with 0) when icon clicked at non-zero volume', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderVolumeControl({ onChange });
    await user.click(screen.getByText('\uD83D\uDD0A'));
    expect(onChange).toHaveBeenCalledWith(0);
  });

  it('unmutes (calls onChange with 100) when icon clicked at zero volume', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderVolumeControl({ value: 0, onChange });
    await user.click(screen.getByText('\uD83D\uDD07'));
    expect(onChange).toHaveBeenCalledWith(100);
  });

  it('shows the selected output name in collapsed mode without device controls', () => {
    renderVolumeControl();

    expect(screen.getByText('All Speakers (2) - Desk Speakers')).toHaveClass('section-label');
    expect(screen.getByTitle('Show output speakers')).toBeInTheDocument();
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
    expect(screen.queryByTitle('Rename MacBook Pro Speakers')).not.toBeInTheDocument();
  });

  /** Speaker expansion uses its own section toggle callback. */
  it('requests speaker expansion from its own toggle', async () => {
    const user = userEvent.setup();
    const onToggleExpanded = vi.fn();
    renderVolumeControl({ onToggleExpanded });

    await user.click(screen.getByTitle('Show output speakers'));

    expect(onToggleExpanded).toHaveBeenCalledOnce();
  });

  it('keeps the collapsed heading readonly; renames happen in expanded mode', async () => {
    const user = userEvent.setup();
    const onRenameOutput = vi.fn();
    renderVolumeControl({ onRenameOutput });

    await user.click(screen.getByText('All Speakers (2) - Desk Speakers'));
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(onRenameOutput).not.toHaveBeenCalled();
  });

  it('shows every output row including the selected device in expanded mode', () => {
    renderVolumeControl({ expanded: true });

    expect(screen.getByText('All Speakers (2)')).toHaveClass('section-label');
    expect(screen.getByTitle('Hide output speakers')).toBeInTheDocument();
    expect(
      screen.queryByTitle('Rename active output MacBook Pro Speakers'),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Select Desk Speakers' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Select Headphones' })).not.toBeChecked();
    expect(screen.getByTitle('Rename MacBook Pro Speakers')).toBeInTheDocument();
    expect(screen.getByTitle('Rename USB Headphones')).toBeInTheDocument();
  });

  it('renames the selected output from its list row', async () => {
    const user = userEvent.setup();
    const onRenameOutput = vi.fn();
    renderVolumeControl({ expanded: true, onRenameOutput });

    await user.click(screen.getByTitle('Rename MacBook Pro Speakers'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'Conference Speakers{Enter}');

    expect(onRenameOutput).toHaveBeenCalledWith('speakers', 'Conference Speakers');
  });

  it('selects an output from its dedicated radio button', async () => {
    const user = userEvent.setup();
    const onSelectOutput = vi.fn();
    renderVolumeControl({ expanded: true, onSelectOutput });

    await user.click(screen.getByRole('radio', { name: 'Select Headphones' }));

    expect(onSelectOutput).toHaveBeenCalledWith('headphones');
  });

  it('renames an available output by clicking its name and committing the textbox', async () => {
    const user = userEvent.setup();
    const onRenameOutput = vi.fn();
    renderVolumeControl({ expanded: true, onRenameOutput });

    await user.click(screen.getByTitle('Rename USB Headphones'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'Office Speakers{Enter}');

    expect(onRenameOutput).toHaveBeenCalledWith('headphones', 'Office Speakers');
  });

  it('cancels an output rename on Escape', async () => {
    const user = userEvent.setup();
    const onRenameOutput = vi.fn();
    renderVolumeControl({ expanded: true, onRenameOutput });

    await user.click(screen.getByTitle('Rename USB Headphones'));
    await user.type(screen.getByRole('textbox'), ' changed{Escape}');

    expect(onRenameOutput).not.toHaveBeenCalled();
    expect(screen.getByTitle('Rename USB Headphones')).toBeInTheDocument();
  });

  it('disables output selection while another output switch is in progress', () => {
    renderVolumeControl({ expanded: true, updatingDeviceId: 'headphones' });

    expect(screen.getByRole('radio', { name: 'Select Headphones' })).toBeDisabled();
  });

  it('shows reorder controls without state selectors in the main panel', async () => {
    const user = userEvent.setup();
    const onMoveOutput = vi.fn();
    renderVolumeControl({ expanded: true, onMoveOutput });

    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(screen.getByTitle('Move Desk Speakers up')).toBeDisabled();
    expect(screen.getByTitle('Move Headphones down')).toBeDisabled();

    await user.click(screen.getByTitle('Move Headphones up'));

    expect(onMoveOutput).toHaveBeenCalledWith('headphones', 'up');
  });

  it('disables disabled outputs and omits hidden outputs', () => {
    renderVolumeControl({
      expanded: true,
      outputState: {
        devices: [
          {
            id: 'speakers',
            name: 'MacBook Pro Speakers',
            originalName: 'MacBook Pro Speakers',
            state: 'enabled',
            isBuiltIn: true,
          },
          {
            id: 'teams',
            name: 'Microsoft Teams Audio',
            originalName: 'Microsoft Teams Audio',
            state: 'disabled',
            isBuiltIn: false,
          },
          {
            id: 'zoom',
            name: 'ZoomAudioDevice',
            originalName: 'ZoomAudioDevice',
            state: 'hidden',
            isBuiltIn: false,
          },
        ],
        selectedDeviceId: 'speakers',
      },
    });

    expect(screen.getByRole('radio', { name: 'Select Microsoft Teams Audio' })).toBeDisabled();
    expect(screen.queryByRole('radio', { name: 'Select ZoomAudioDevice' })).not.toBeInTheDocument();

    expect(screen.queryByRole('radio', { name: 'Select ZoomAudioDevice' })).not.toBeInTheDocument();
  });
});

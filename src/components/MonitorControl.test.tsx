import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import MonitorControl from './MonitorControl';
import { Monitor } from '../types';

const externalMonitor: Monitor = {
  id: '1',
  uid: '1::Dell U2723QE',
  name: 'Dell U2723QE',
  originalName: 'Dell U2723QE',
  brightness: 80,
  supportsBrightness: true,
  isBuiltIn: false,
};

const builtInMonitor: Monitor = {
  id: 'builtin',
  uid: 'builtin::Built-in Display',
  name: 'Built-in Display',
  originalName: 'Built-in Display',
  brightness: 60,
  supportsBrightness: true,
  isBuiltIn: true,
};

describe('MonitorControl', () => {
  it('renders the monitor name', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
  });

  it('renders brightness slider', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    const sliders = screen.getAllByRole('slider');
    expect(sliders.length).toBeGreaterThanOrEqual(1);
    expect(sliders[0]).toHaveValue('80');
  });

  it('enters edit mode on name click', async () => {
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );

    await user.click(screen.getByText('Dell U2723QE'));
    const input = screen.getByRole('textbox');
    expect(input).toBeInTheDocument();
    expect(input).toHaveValue('Dell U2723QE');
  });

  it('calls onRename when editing is confirmed with Enter', async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={onRename}
        minBrightness={10}
      />,
    );

    await user.click(screen.getByText('Dell U2723QE'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'My Monitor{Enter}');
    expect(onRename).toHaveBeenCalledWith('My Monitor');
  });

  it('cancels editing on Escape', async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={onRename}
        minBrightness={10}
      />,
    );

    await user.click(screen.getByText('Dell U2723QE'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'New Name{Escape}');
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
  });

  it('does not call onRename when name is unchanged', async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={onRename}
        minBrightness={10}
      />,
    );

    await user.click(screen.getByText('Dell U2723QE'));
    const input = screen.getByRole('textbox');
    await user.type(input, '{Enter}');
    expect(onRename).not.toHaveBeenCalled();
  });

  it('shows monitor icon for external display', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    expect(screen.getByText('\uD83D\uDDA5')).toBeInTheDocument();
  });

  it('shows laptop icon for built-in display', () => {
    render(
      <MonitorControl
        monitor={builtInMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    expect(screen.getByText('\uD83D\uDCBB')).toBeInTheDocument();
  });

  it('renders reorder buttons when callbacks provided', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        isFirst={false}
        isLast={false}
        minBrightness={10}
      />,
    );
    expect(screen.getByTitle('Move up')).toBeInTheDocument();
    expect(screen.getByTitle('Move down')).toBeInTheDocument();
    expect(screen.getByTitle('Move up')).not.toBeDisabled();
    expect(screen.getByTitle('Move down')).not.toBeDisabled();
  });

  it('disables move up for first monitor and move down for last', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        isFirst={true}
        isLast={true}
        minBrightness={10}
      />,
    );
    expect(screen.getByTitle('Move up')).toBeDisabled();
    expect(screen.getByTitle('Move down')).toBeDisabled();
  });

  it('calls onMoveUp and onMoveDown when clicked', async () => {
    const onMoveUp = vi.fn();
    const onMoveDown = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        onMoveUp={onMoveUp}
        onMoveDown={onMoveDown}
        isFirst={false}
        isLast={false}
        minBrightness={10}
      />,
    );

    await user.click(screen.getByTitle('Move up'));
    expect(onMoveUp).toHaveBeenCalledTimes(1);

    await user.click(screen.getByTitle('Move down'));
    expect(onMoveDown).toHaveBeenCalledTimes(1);
  });

  it('does not render reorder buttons without callbacks', () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    expect(screen.queryByTitle('Move up')).not.toBeInTheDocument();
    expect(screen.queryByTitle('Move down')).not.toBeInTheDocument();
  });

  it('shows originalName as fallback when name is empty', () => {
    const monitorWithEmptyName: Monitor = {
      ...externalMonitor,
      name: '',
      originalName: 'Dell U2723QE',
    };
    render(
      <MonitorControl
        monitor={monitorWithEmptyName}
        onBrightnessChange={() => {}}
        onRename={() => {}}
        minBrightness={10}
      />,
    );
    expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
  });
});

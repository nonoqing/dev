// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { TelemetryConfigSection } from './TelemetryConfigSection';

const getTelemetryStateMock = vi.hoisted(() => vi.fn());
const setTelemetryLevelMock = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getTelemetryState: getTelemetryStateMock,
    setTelemetryLevel: setTelemetryLevelMock,
  },
}));

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

const health = (effectiveLevel: 'off' | 'basic' | 'diagnostic') => ({
  state: effectiveLevel === 'off' ? 'closed' : 'healthy',
  userLevel: effectiveLevel,
  effectiveLevel,
  generation: 0,
  queuedRecords: 0,
  queuedBytes: 0,
  inFlightBatches: 0,
  retryAttempts: 0,
  locallyDropped: 0,
  ambiguous: 0,
  acknowledged: 0,
  serverRejected: 0,
  lastSuccessUnixMs: null,
});

describe('TelemetryConfigSection', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Object.defineProperty(window, '__TAURI__', { configurable: true, value: {} });
    getTelemetryStateMock.mockReset().mockResolvedValue({ level: 'off', health: health('off') });
    setTelemetryLevelMock.mockReset().mockResolvedValue({
      level: 'basic',
      health: health('basic'),
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    delete (window as Window & { __TAURI__?: unknown }).__TAURI__;
    container.remove();
  });

  it('loads the redacted state and updates only the selected level', async () => {
    await act(async () => {
      root.render(<TelemetryConfigSection />);
    });

    const select = container.querySelector<HTMLElement>('[role="combobox"]');
    expect(select).not.toBeNull();
    expect(select?.textContent).toContain('telemetry.levels.off');

    await act(async () => {
      select?.click();
    });

    const basicOption = document.querySelector<HTMLElement>('[role="option"]:nth-child(2)');
    expect(basicOption?.textContent).toContain('telemetry.levels.basic');

    await act(async () => {
      basicOption?.click();
    });

    expect(setTelemetryLevelMock).toHaveBeenCalledWith('basic');
    expect(container.textContent).not.toContain('endpoint');
    expect(container.textContent).not.toContain('secret');
    expect(container.textContent).not.toContain('installation');
  });

});

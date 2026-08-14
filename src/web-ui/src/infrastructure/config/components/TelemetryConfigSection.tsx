import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfigPageMessage, Select, confirmDialog } from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import type { TelemetryLevel, TelemetryState } from '../types';
import { ConfigPageRow, ConfigPageSection } from './common';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TelemetryConfigSection');
export const TelemetryConfigSection: React.FC = () => {
  const { t } = useTranslation('settings/basics');
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  const [state, setState] = useState<TelemetryState | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{
    type: 'success' | 'error' | 'info';
    text: string;
  } | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    void configAPI.getTelemetryState()
      .then((next) => {
        if (!cancelled) setState(next);
      })
      .catch((error) => {
        log.error('Failed to load telemetry state', error);
        if (!cancelled) {
          setMessage({ type: 'error', text: t('telemetry.messages.loadFailed') });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [isTauri, t]);

  const setLevel = useCallback(async (level: TelemetryLevel) => {
    if (!state || level === state.level || saving) return;
    let sensitiveContentConsent = state.sensitiveContentConsent;
    if (level === 'debug' && !sensitiveContentConsent) {
      sensitiveContentConsent = await confirmDialog({
        title: t('telemetry.debugConsent.title'),
        message: t('telemetry.debugConsent.message'),
        confirmText: t('telemetry.debugConsent.confirm'),
        cancelText: t('telemetry.debugConsent.cancel'),
        type: 'warning',
        confirmDanger: true,
      });
      if (!sensitiveContentConsent) return;
    }
    const previous = state;
    setState({ ...state, level, sensitiveContentConsent });
    setSaving(true);
    setMessage(null);
    try {
      const next = await configAPI.setTelemetryLevel(level, sensitiveContentConsent);
      setState(next);
      const unavailable = next.level !== 'off' && next.health.effectiveLevel === 'off';
      setMessage({
        type: unavailable ? 'info' : 'success',
        text: t(unavailable ? 'telemetry.messages.unavailable' : 'telemetry.messages.saved'),
      });
    } catch (error) {
      log.error('Failed to update telemetry level', { level, error });
      setState(previous);
      setMessage({ type: 'error', text: t('telemetry.messages.saveFailed') });
    } finally {
      setSaving(false);
    }
  }, [saving, state, t]);

  if (!isTauri) return null;

  return (
    <ConfigPageSection title={t('telemetry.title')} description={t('telemetry.hint')}>
      <ConfigPageMessage message={message} />
      <ConfigPageRow
        label={t('telemetry.levelLabel')}
        description={t('telemetry.levelDescription')}
        align="center"
      >
        <Select
          value={state?.level ?? 'off'}
          onChange={(value) => { void setLevel(value as TelemetryLevel); }}
          options={[
            { value: 'off', label: t('telemetry.levels.off') },
            { value: 'basic', label: t('telemetry.levels.basic') },
            { value: 'diagnostic', label: t('telemetry.levels.diagnostic') },
            { value: 'debug', label: t('telemetry.levels.debug') },
          ]}
          disabled={!state || saving}
          triggerAriaLabel={t('telemetry.levelLabel')}
        />
      </ConfigPageRow>
    </ConfigPageSection>
  );
};

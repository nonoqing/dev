import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import { configManager } from './ConfigManager';
import type { PermissionEffect, PermissionRule, ToolPermissionConfig } from '../types';

const log = createLogger('PermissionConfig');
const CONFIG_PATH = 'tool_permissions';

export const DEFAULT_TOOL_PERMISSION_CONFIG: ToolPermissionConfig = {
  default_permission: 'ask',
  policy: {
    preset: 'ask',
    rules: [],
  },
  interaction: {
    auto_approve_ask: false,
  },
};

function normalizeRule(value: unknown): PermissionRule | null {
  if (!value || typeof value !== 'object') return null;
  const rule = value as Partial<PermissionRule>;
  const effect: PermissionEffect = rule.effect === 'allow' || rule.effect === 'deny' ? rule.effect : 'ask';
  if (typeof rule.action !== 'string' || typeof rule.resource !== 'string') return null;
  return { action: rule.action, resource: rule.resource, effect };
}

export function normalizeToolPermissionConfig(value: unknown): ToolPermissionConfig {
  const input = value && typeof value === 'object'
    ? value as { default_permission?: unknown; policy?: { preset?: unknown; rules?: unknown }; interaction?: { auto_approve_ask?: unknown } }
    : {};
  const policy = input.policy ?? {};
  const interaction = input.interaction ?? {};
  const hasDefaultPermission = Object.prototype.hasOwnProperty.call(input, 'default_permission');
  const hasValidDefaultPermission = input.default_permission === 'ask'
    || input.default_permission === 'allow'
    || input.default_permission === 'deny';
  const defaultPermission: PermissionEffect = input.default_permission === 'allow' || input.default_permission === 'deny'
    ? input.default_permission
    : hasDefaultPermission
      ? 'ask'
      : policy.preset === 'full_access'
      ? 'allow'
      : policy.preset === 'deny'
        ? 'deny'
        : 'ask';
  const rules = Array.isArray(policy.rules)
    ? policy.rules.map(normalizeRule).filter((rule): rule is PermissionRule => rule !== null)
    : [];

  return {
    default_permission: defaultPermission,
    policy: {
      preset: hasDefaultPermission && !hasValidDefaultPermission
        ? 'ask'
        : policy.preset === 'full_access' || policy.preset === 'deny' ? policy.preset : 'ask',
      rules,
    },
    interaction: {
      auto_approve_ask: hasDefaultPermission && !hasValidDefaultPermission
        ? false
        : interaction.auto_approve_ask === true,
    },
  };
}

export class PermissionConfigService {
  async getConfig(): Promise<ToolPermissionConfig> {
    try {
      return normalizeToolPermissionConfig(await configManager.getConfig<ToolPermissionConfig>(CONFIG_PATH));
    } catch (error) {
      log.warn('Failed to load tool permission config, using safe defaults', error);
      return {
        default_permission: DEFAULT_TOOL_PERMISSION_CONFIG.default_permission,
        policy: { preset: DEFAULT_TOOL_PERMISSION_CONFIG.policy.preset, rules: [] },
        interaction: { auto_approve_ask: DEFAULT_TOOL_PERMISSION_CONFIG.interaction.auto_approve_ask },
      };
    }
  }

  async saveConfig(config: ToolPermissionConfig): Promise<ToolPermissionConfig> {
    const normalized = normalizeToolPermissionConfig(config);
    await configManager.setConfig(CONFIG_PATH, normalized);
    globalEventBus.emit('permission:config:updated', normalized);
    return normalized;
  }

  async setPreset(preset: ToolPermissionConfig['policy']['preset']): Promise<ToolPermissionConfig> {
    await configManager.setConfig(`${CONFIG_PATH}.policy.preset`, preset);
    await configManager.setConfig(
      `${CONFIG_PATH}.default_permission`,
      preset === 'full_access' ? 'allow' : preset === 'deny' ? 'deny' : 'ask',
    );
    globalEventBus.emit('permission:config:updated');
    return this.getConfig();
  }

  async setAutoApproveAsk(enabled: boolean): Promise<ToolPermissionConfig> {
    await configManager.setConfig(`${CONFIG_PATH}.interaction.auto_approve_ask`, enabled);
    globalEventBus.emit('permission:config:updated');
    return this.getConfig();
  }
}

export const permissionConfigService = new PermissionConfigService();

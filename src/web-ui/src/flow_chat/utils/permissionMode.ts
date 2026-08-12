import type { ToolPermissionConfig } from '@/infrastructure/config/types';
import type { SessionPermissionMode } from '@/infrastructure/api/service-api/AgentAPI';
import type { ChatInputPermissionMode } from '../components/ChatInputWorkspaceStrip';

/**
 * The chat input control and the backend name the same three modes slightly
 * differently: the control has carried `auto` since before the backend had a
 * single mode value, and the backend spells it `auto_approve`. These helpers
 * keep that one difference in one place instead of at every call site.
 */

type NativePermissionMode = Exclude<ChatInputPermissionMode, 'acp'>;

/** Backend mode -> chat input control mode. */
export function chatInputPermissionMode(mode: SessionPermissionMode): NativePermissionMode {
  return mode === 'auto_approve' ? 'auto' : mode === 'deny' ? 'reject' : mode;
}

/** Chat input control mode -> backend mode. */
export function sessionPermissionMode(mode: NativePermissionMode): SessionPermissionMode {
  return mode === 'auto' ? 'auto_approve' : mode === 'reject' ? 'deny' : mode;
}

/**
 * Derives the mode a stored configuration represents.
 *
 * Mirrors `PermissionMode::from_config`: full access already resolves every
 * ask, so it outranks the auto-approve preference.
 */
export function permissionModeFromConfig(config: ToolPermissionConfig): SessionPermissionMode {
  if (config.default_permission === 'deny') return 'deny';
  if (config.default_permission === 'allow') return 'full_access';
  return config.interaction.auto_approve_ask ? 'auto_approve' : 'ask';
}

/** Projects a mode back onto the two stored configuration knobs. */
export function permissionModeToConfig(
  config: ToolPermissionConfig,
  mode: SessionPermissionMode,
): ToolPermissionConfig {
  return {
    default_permission: mode === 'full_access' ? 'allow' : mode === 'deny' ? 'deny' : 'ask',
    policy: {
      ...config.policy,
      preset: mode === 'full_access' ? 'full_access' : mode === 'deny' ? 'deny' : 'ask',
    },
    interaction: {
      ...config.interaction,
      auto_approve_ask: mode === 'auto_approve',
    },
  };
}

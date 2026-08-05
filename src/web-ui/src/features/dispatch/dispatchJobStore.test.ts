/**
 * @vitest-environment jsdom
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dispatchJobStore } from './dispatchJobStore';
import type { OutboundDispatchRecord } from './types';

function outboundRecord(index: number | 'pending'): OutboundDispatchRecord {
  return {
    jobId: `job-${index}`,
    sessionId: `session-${index}`,
    target: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
      displayName: 'build-host',
    },
    sourceWorkspacePath: '/source',
    workspacePath: '/repo',
    promptPreview: 'Durable dispatch',
    lastCursor: 10,
    lastState: 'succeeded',
    createdAt: '2026-07-28T00:00:00Z',
    updatedAt: '2026-07-28T00:00:01Z',
  };
}

function registerJob(state: 'running' | 'succeeded' = 'running'): void {
  dispatchJobStore.getState().registerJob({
    jobId: 'job-1',
    sessionId: 'session-1',
    targetRequest: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
    },
    target: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
      displayName: 'build-host',
    },
    sourceWorkspacePath: '/source',
    title: 'Dispatch test',
    agentType: 'agentic',
    approvalPolicy: 'reject-and-report',
    cursor: 10,
    state,
    terminalDrained: state === 'succeeded',
    appliedEventIds: [],
    pendingPermissions: [],
    eventLogComplete: true,
    historyTruncated: false,
    omittedEventCount: 0,
    createdAt: 1,
    updatedAt: 1,
  });
}

describe('dispatchJobStore', () => {
  beforeEach(() => {
    vi.spyOn(console, 'info').mockImplementation(() => {});
    dispatchJobStore.getState().clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('keeps cursors monotonic and clears terminal-drained state on progress', () => {
    registerJob();
    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 20,
      terminalDrained: true,
    });
    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 12,
    });

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      cursor: 20,
      state: 'running',
      terminalDrained: true,
    });

    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 21,
    });
    expect(dispatchJobStore.getState().jobs['job-1'].terminalDrained).toBe(false);
  });

  it('never regresses a terminal state from a stale status or outbound record', () => {
    registerJob('succeeded');
    dispatchJobStore.getState().updateProgress('job-1', {
      state: 'running',
      cursor: 11,
    });
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 9,
      lastState: 'queued',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'succeeded',
      cursor: 11,
    });
  });

  it('reopens the drain gate when the target accepts a follow-up turn', () => {
    registerJob('succeeded');

    dispatchJobStore.getState().markFollowUpAccepted('job-1', 'queued');

    // The follow-up resumes the event log rather than replaying it, so the
    // cursor and applied events survive while the terminal pin does not.
    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'queued',
      cursor: 10,
      terminalDrained: false,
    });
  });

  it('reopens the drain gate even when a retried follow-up already finished', () => {
    registerJob('succeeded');

    // A retried continue can report a terminal state when the turn ran to
    // completion before the retry resolved. The reported state applies as-is,
    // but the gate must still reopen so the missed pages get drained.
    dispatchJobStore.getState().markFollowUpAccepted('job-1', 'succeeded');

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'succeeded',
      terminalDrained: false,
    });
  });

  it('keeps the renderer cursor independent from controller-wide observer progress', () => {
    registerJob();
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/canonical/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/canonical/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 900,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      cursor: 10,
      target: {
        workspacePath: '/canonical/repo',
      },
    });
  });

  it('reconstructs immutable submission metadata from the durable outbound index', () => {
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-restored',
      sessionId: 'session-restored',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      workspacePath: '/repo',
      promptPreview: 'Prompt preview',
      title: 'Remote review',
      agentType: 'debug',
      approvalPolicy: 'remote',
      model: 'configured-model',
      reasoningPreset: 'high',
      sourceWorkspacePath: '/controller/repo',
      sourceWorkspaceId: 'workspace-1',
      baselineWorktreeId: 'worktree-1',
      baselineWorktreePath: '/controller/.bitfun/worktrees/baseline',
      baseCommit: 'abc123',
      branch: 'bitfun/dispatch/job-rest',
      remoteUrl: 'git@example.test:team/repo.git',
      syncedHeadCommit: 'def456',
      lastCursor: 900,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-restored']).toMatchObject({
      title: 'Remote review',
      agentType: 'debug',
      approvalPolicy: 'remote',
      model: 'configured-model',
      reasoningPreset: 'high',
      sourceWorkspacePath: '/controller/repo',
      sourceWorkspaceId: 'workspace-1',
      branch: 'bitfun/dispatch/job-rest',
      baselineWorktreePath: '/controller/.bitfun/worktrees/baseline',
      syncedHeadCommit: 'def456',
      cursor: 0,
    });
  });

  it('hydrates Git sync metadata into an existing pre-ack job', () => {
    registerJob();

    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      baselineWorktreePath: '/source/.bitfun/worktrees/baseline',
      branch: 'bitfun/dispatch/job-1',
      syncedHeadCommit: 'def456',
      workspacePath: '/target/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 0,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      branch: 'bitfun/dispatch/job-1',
      baselineWorktreePath: '/source/.bitfun/worktrees/baseline',
      syncedHeadCommit: 'def456',
    });
  });

  it('hydrates a follow-up reasoning preset from the durable outbound index', () => {
    registerJob();
    dispatchJobStore.getState().updateReasoningPreset('job-1', 'low');
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      reasoningPreset: 'auto',
      lastCursor: 10,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1'].reasoningPreset).toBe('auto');

    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Legacy dispatch record',
      lastCursor: 10,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:02Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1'].reasoningPreset).toBe('auto');
  });

  it('marks a missing baseline worktree without changing job execution state', () => {
    registerJob();

    dispatchJobStore.getState().setBaselineWorktreeMissing('job-1', true);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'running',
      baselineWorktreeMissing: true,
    });
  });

  it('drops a legacy outbound job instead of guessing its source workspace', () => {
    const record = {
      jobId: 'job-restored',
      sessionId: 'session-restored',
      target: {
        kind: 'ssh' as const,
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
        displayName: 'build-host',
      },
      workspacePath: '/target/repo',
      promptPreview: 'Prompt preview',
      lastCursor: 0,
      lastState: 'running' as const,
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    };

    // Simulate a cache polluted by the old current-workspace fallback.
    dispatchJobStore.getState().registerJob({
      jobId: 'job-restored',
      sessionId: 'session-restored',
      targetRequest: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
      },
      target: record.target,
      sourceWorkspacePath: '/wrong/current/workspace',
      title: 'Prompt preview',
      agentType: 'agentic',
      approvalPolicy: 'reject-and-report',
      cursor: 0,
      state: 'running',
      appliedEventIds: [],
      pendingPermissions: [],
      eventLogComplete: true,
      historyTruncated: false,
      omittedEventCount: 0,
      createdAt: 1,
      updatedAt: 1,
    });

    dispatchJobStore.getState().mergeOutboundRecords([record]);
    expect(dispatchJobStore.getState().jobs['job-restored']).toBeUndefined();
    expect(
      dispatchJobStore.getState().transportByJobId['job-restored'],
    ).toBeUndefined();
  });

  it('uses the stable baseline project when a linked source checkout is unavailable', () => {
    const record = {
      jobId: 'job-stable-project',
      sessionId: 'session-stable-project',
      target: {
        kind: 'ssh' as const,
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
        displayName: 'build-host',
      },
      baselineProjectWorkspacePath: '/controller/main-project',
      baselineWorktreeId: 'worktree-1',
      baselineWorktreePath: '/controller/baselines/job-stable-project',
      branch: 'bitfun/dispatch/job-stable-project',
      workspacePath: '/target/repo',
      promptPreview: 'Prompt preview',
      lastCursor: 0,
      lastState: 'running' as const,
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    };

    dispatchJobStore.getState().mergeOutboundRecords([record]);

    expect(
      dispatchJobStore.getState().jobs['job-stable-project']?.sourceWorkspacePath,
    ).toBe('/controller/main-project');
  });

  it('drops acknowledged renderer cache missing from the controller index', () => {
    registerJob();
    dispatchJobStore.getState().mergeOutboundRecords([]);

    expect(dispatchJobStore.getState().jobs['job-1']).toBeUndefined();
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toBeUndefined();
  });

  it('keeps a pre-ack job while the controller index has no record yet', () => {
    registerJob();
    dispatchJobStore.getState().registerJob({
      ...dispatchJobStore.getState().jobs['job-1'],
      state: 'submitting',
    });
    dispatchJobStore.getState().mergeOutboundRecords([]);

    expect(dispatchJobStore.getState().jobs['job-1']?.state).toBe('submitting');
  });

  it('persists a dismissal tombstone so reconciliation cannot reopen the projection', () => {
    registerJob();
    dispatchJobStore.getState().dismissJob('job-1');
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 10,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-1');
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-1');
  });

  it('uses a session tombstone when deletion happens before the job id is known', () => {
    dispatchJobStore.getState().dismissSession('session-late');
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-late',
      sessionId: 'session-late',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 0,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-late']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-late');
  });

  it('never evicts a tombstone while its durable outbound record still exists', () => {
    for (let index = 0; index <= 2048; index += 1) {
      dispatchJobStore.getState().dismissSession(
        `session-${index}`,
        `job-${index}`,
      );
    }

    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-0',
      sessionId: 'session-0',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Old durable dispatch',
      lastCursor: 10,
      lastState: 'succeeded',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-0']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-0');
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-0');
    expect(dispatchJobStore.getState().dismissedJobIds).toHaveLength(2049);
    expect(dispatchJobStore.getState().dismissedSessionIds).toHaveLength(2049);
  });

  it('keeps pre-ack tombstones when authoritative tombstones fill the retention budget', () => {
    const records = Array.from({ length: 2048 }, (_, index) => outboundRecord(index));
    for (let index = 0; index < records.length; index += 1) {
      dispatchJobStore.getState().dismissSession(
        `session-${index}`,
        `job-${index}`,
      );
    }
    dispatchJobStore.getState().mergeOutboundRecords(records);

    dispatchJobStore.getState().dismissSession('session-pending', 'job-pending');
    dispatchJobStore.getState().mergeOutboundRecords(records);

    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-pending');
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-pending');

    dispatchJobStore.getState().mergeOutboundRecords([
      ...records,
      outboundRecord('pending'),
    ]);

    expect(dispatchJobStore.getState().jobs['job-pending']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-pending');
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-pending');
  });

  it('keeps transport reachability transient and separate from authoritative job state', () => {
    registerJob();
    dispatchJobStore.getState().setTransportState(
      'job-1',
      'unreachable',
      'SSH target is offline',
    );

    expect(dispatchJobStore.getState().jobs['job-1'].state).toBe('running');
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toEqual({
      reachability: 'unreachable',
      lastTransportError: 'SSH target is offline',
    });

    const partialize = dispatchJobStore.persist.getOptions().partialize;
    const persistedState = partialize?.(
      dispatchJobStore.getState(),
    ) as Record<string, unknown> | undefined;
    expect(persistedState?.transportByJobId).toBeUndefined();
    expect(persistedState?.dismissedSessionIds).toEqual([]);
  });

  it('resets reasoning to Auto when the dispatch model changes', () => {
    registerJob();
    dispatchJobStore.getState().updateReasoningPreset('job-1', 'high');

    dispatchJobStore.getState().updateModel('job-1', 'model-2');

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      model: 'model-2',
      reasoningPreset: 'auto',
    });
  });

  it('persists the target reasoning preset in observer state', () => {
    registerJob();
    dispatchJobStore.getState().updateReasoningPreset('job-1', 'high');

    const partialize = dispatchJobStore.persist.getOptions().partialize;
    const persistedState = partialize?.(
      dispatchJobStore.getState(),
    ) as { jobs?: Record<string, { reasoningPreset?: string }> } | undefined;

    expect(persistedState?.jobs?.['job-1']?.reasoningPreset).toBe('high');
  });
});

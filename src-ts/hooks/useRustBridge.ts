import { logger } from '../utils/logger.js';
import type { CoreConfirmDecision, CoreEvent, CoreConfirmationRequest, AvailableModel, LatencyInfo, AppConfig } from '../types/index.js';
import { loadCoreApi } from '../utils/loadCoreApi.js';

const coreapi = loadCoreApi();

export interface AgentDesc {
  sessionId: string;
  createdAt: number;
  lastActiveAt: number;
}

export interface SavedSessionInfo {
  sessionId: string;
  createdAtMs: number;
  updatedAtMs: number;
  messageCount: number;
}

export interface ProviderMessage {
  role: string;
  content: string;
}

// Session management maps
const sessionDescs = new Map<string, AgentDesc>();
const sessionInstances = new Map<string, any>();

export function useRustBridge() {
  function createSessionId(): string {
    return coreapi.createSessionId();
  }

  function getAppConfig(): AppConfig {
    const jsonStr = coreapi.getAppConfig();
    try {
      return JSON.parse(jsonStr);
    } catch (e) {
      logger.error('Failed to parse app config', e);
      return {};
    }
  }

  function listAvailableModels(): AvailableModel[] {
    return coreapi.listAvailableModels();
  }

  function getDefaultModel(): string | null {
    return coreapi.getDefaultModel();
  }

  function getSession(sessionId: string) {
    let session = sessionInstances.get(sessionId);
    if (!session) {
        session = coreapi.Session.open(sessionId);
        sessionInstances.set(sessionId, session);
        sessionDescs.set(sessionId, {
            sessionId,
            createdAt: Date.now(),
            lastActiveAt: Date.now(),
        });
        logger.info(`Agent session initialized: ${sessionId}`);
    }
    return session;
  }

  async function getAvailableModels(sessionId: string): Promise<AvailableModel[]> {
      const session = getSession(sessionId);
      return session.getAvailableModels();
  }

  async function setModel(sessionId: string, provider: string, model: string): Promise<void> {
      const session = getSession(sessionId);
      return session.setModel(provider, model);
  }

  async function getSessions(): Promise<string[]> {
      return coreapi.Session.getSessions();
  }

  async function getSavedSessions(): Promise<SavedSessionInfo[]> {
      return coreapi.Session.getSavedSessions();
  }

  async function getSessionHistory(sessionId: string): Promise<ProviderMessage[]> {
      const session = getSession(sessionId);
      return session.getHistory();
  }

  async function setTheme(theme: string): Promise<void> {
      return coreapi.Session.setTheme(theme);
  }

  function getAgentMode(sessionId: string): 'plan' | 'build' {
      const session = getSession(sessionId);
      return session.getAgentMode();
  }

  async function setAgentMode(sessionId: string, mode: 'plan' | 'build'): Promise<void> {
      const session = getSession(sessionId);
      return session.setAgentMode(mode);
  }

  function getApprovalMode(sessionId: string): 'read-only' | 'agent' | 'agent-full' {
      const session = getSession(sessionId);
      return session.getApprovalMode();
  }

  async function setApprovalMode(sessionId: string, mode: 'read-only' | 'agent' | 'agent-full'): Promise<void> {
      const session = getSession(sessionId);
      return session.setApprovalMode(mode);
  }

  async function checkLatency(sessionId: string): Promise<LatencyInfo> {
      const session = getSession(sessionId);
      return session.checkLatency();
  }

  async function askAgent(
    sessionId: string, 
    prompt: string, 
    onEvent?: (event: CoreEvent) => void,
    onConfirmation?: (request: CoreConfirmationRequest) => void
  ): Promise<string> {
    const session = getSession(sessionId);
    
    // Update last active time
    const desc = sessionDescs.get(sessionId);
    if (desc) {
        desc.lastActiveAt = Date.now();
        sessionDescs.set(sessionId, desc);
    }
    
      const debugEvents = process.env.CARRYCODE_DEBUG_EVENTS === '1';
    let eventCount = 0;
    let nullEventCount = 0;
    let errorCount = 0;

    const summarizeEvent = (event: CoreEvent): string => {
      const textLen = typeof (event as any).text === 'string' ? (event as any).text.length : 0;
      const hasConfirm = Boolean((event as any).confirm);
      const toolName = typeof (event as any).toolName === 'string' ? (event as any).toolName : '';
      const argsLen = typeof (event as any).argsSummary === 'string' ? (event as any).argsSummary.length : 0;
      const respLen = typeof (event as any).responseSummary === 'string' ? (event as any).responseSummary.length : 0;
      const displayLen = typeof (event as any).displayText === 'string' ? (event as any).displayText.length : 0;
      return `type=${String((event as any).eventType)} stage=${String(
        (event as any).stage ?? '',
      )} op=${String((event as any).toolOperation ?? '')} tool=${toolName} textLen=${textLen} argsLen=${argsLen} respLen=${respLen} displayLen=${displayLen} confirm=${hasConfirm ? 1 : 0}`;
    };

    const noop = (_event: CoreEvent) => {};
    const onUnifiedEvent = (err: any, event?: CoreEvent | null) => {
      if (err) {
        errorCount += 1;
        if (debugEvents && errorCount <= 20) {
          logger.warn(
            `event(unified) err session=${sessionId} err=${String(err?.message ?? err)}`,
          );
        }
        return;
      }
      if (!event) {
        nullEventCount += 1;
        if (debugEvents && nullEventCount <= 5) {
          logger.warn(`event(unified)=null session=${sessionId} nullCount=${nullEventCount}`);
        }
        return;
      }
      eventCount += 1;
      if (debugEvents) {
        logger.debug(`event(unified)#${eventCount} session=${sessionId} ${summarizeEvent(event)}`);
      }
      (typeof onEvent === 'function' ? onEvent : noop)(event);

      const eventType = (event as any).eventType;
      if (eventType === 'ConfirmationRequested' && event.confirm) {
        if (onConfirmation) onConfirmation(event.confirm);
      }
    };

    if (debugEvents) {
      logger.info(`subscribe session=${sessionId} promptChars=${prompt.length}`);
    }
    session.subscribe(onUnifiedEvent as any);
    try {
      const result = await session.execute(prompt);
      return result.content;
    } finally {
      try {
        session.unsubscribe();
      } catch {
      }
      if (debugEvents) {
        logger.info(
          `unsubscribe session=${sessionId} events=${eventCount} nullEvents=${nullEventCount} errors=${errorCount}`,
        );
      }
    }
  }

  async function confirmTool(sessionId: string, decision: CoreConfirmDecision): Promise<void> {
    const session = sessionInstances.get(sessionId);
    if (session) {
        try {
          await session.confirmTool(decision);
        } catch (e) {
          logger.warn('confirmTool failed', e);
        }
    }
  }

  return {
    createSessionId,
    getAppConfig,
    listAvailableModels,
    getDefaultModel,
    askAgent,
    confirmTool,
    getAvailableModels,
    setModel,
    getSessions,
    getSavedSessions,
    getSessionHistory,
    setTheme,
    getAgentMode,
    setAgentMode,
    getApprovalMode,
    setApprovalMode,
    checkLatency,
    sessionDescs,
    sessionInstances,
  };
}

declare module 'carrycode-coreapi' {
  export function createSessionId(): string;
  export function getAppConfig(): string;
  export function listAvailableModels(): AvailableModel[];
  export function getDefaultModel(): string | null;

  export type ResponseStage = '__THINKING__' | '__ANSWERING__' | '__END__';

  export type ToolOperation = '__EXPLORED__' | '__EDITED__' | '__TODO__' | '__BASH__';

  export type CoreEventType =
    | 'Text'
    | 'StageStart'
    | 'StageEnd'
    | 'ToolStart'
    | 'ToolOutput'
    | 'ToolEnd'
    | 'End'
    | 'ConfirmationRequested'
    | 'Error';

  export interface CoreConfirmationRequest {
    requestId: string;
    toolName: string;
    arguments: string;
    kind: string;
    keyPath: string;
  }

  export interface CoreConfirmDecision {
    requestId: string;
    decision: string;
  }

  export interface CoreEvent {
    protocolVersion: number;
    sessionId: string;
    tsMs: number;
    eventType: CoreEventType;
    seq?: number | null;
    text?: string | null;
    stage?: ResponseStage | null;
    toolOperation?: ToolOperation | null;
    toolName?: string | null;
    keyPath?: string | null;
    kind?: string | null;
    argsSummary?: string | null;
    responseSummary?: string | null;
    displayText?: string | null;
    success?: boolean | null;
    confirm?: CoreConfirmationRequest | null;
    errorMessage?: string | null;
  }

  export interface AgentResult {
    content: string;
    tools_used: boolean;
  }

  export interface AvailableModel {
    provider: string;
    model: string;
  }

  export interface LatencyInfo {
    latencyMs: number;
    modelName: string;
  }

  export interface ProviderMessage {
    role: string;
    content: string;
  }

  export interface SavedSessionInfo {
    sessionId: string;
    createdAtMs: number;
    updatedAtMs: number;
    messageCount: number;
  }

  export class Session {
    static open(sessionId: string): Session;
    static getSavedSessions(): SavedSessionInfo[];
    execute(prompt: string): Promise<AgentResult>;
    clearHistory(): Promise<void>;
    getHistory(): Promise<ProviderMessage[]>;
    confirmTool(decision: CoreConfirmDecision): Promise<void>;
    subscribe(onEvent: (err: unknown, event?: CoreEvent | null) => void): void;
    unsubscribe(): void;
    getAvailableModels(): Promise<AvailableModel[]>;
    setModel(provider: string, model: string): Promise<void>;
    checkLatency(): Promise<LatencyInfo>;
    getAgentMode(): 'plan' | 'build';
    setAgentMode(mode: 'plan' | 'build'): Promise<void>;
    getApprovalMode(): 'read-only' | 'agent' | 'agent-full';
    setApprovalMode(mode: 'read-only' | 'agent' | 'agent-full'): void;
  }
}

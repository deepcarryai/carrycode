declare module 'carrycode-coreapi' {
  export function createSessionId(): string;
  export function getLogDir(): string;
  export function getAppConfig(): string;
  export function listAvailableModels(): AvailableModel[];
  export function getDefaultModel(): string | null;
  export function listProviderPresets(): ProviderPreset[];
  export function getConfigBootstrapState(): ConfigBootstrapState;
  export function setLanguage(language: string): void;
  export function setWelcomeWizardDone(done: boolean): void;
  export function saveUserProviders(providers: UserProviderConfig[]): void;
  export function listAvailableSkills(sessionId: string): SkillManifest[];
  export function getSkillMarkdown(sessionId: string, skillName: string): string;
  export function enableSkillForSession(sessionId: string, skillName: string): Promise<void>;
  export function disableSkillForSession(sessionId: string, skillName: string): Promise<void>;

  export interface SkillManifest {
    name: string;
    description?: string | null;
    argumentHint?: string | null;
    disableModelInvocation?: boolean | null;
    userInvocable?: boolean | null;
    allowedTools?: string[] | null;
    context?: string | null;
    [key: string]: any;
  }

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

  export interface ProviderPreset {
    providerId: string;
    providerBrand: string;
    baseUrl: string;
    apiKey: string;
    modelName: string;
    providerDesc: string;
  }

  export interface ConfigBootstrapState {
    needsWelcomeWizard: boolean;
    runtimeLanguage?: string | null;
  }

  export interface UserProviderConfig {
    providerBrand: string;
    providerId: string;
    modelName: string;
    baseUrl: string;
    apiKey: string;
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
    static open(sessionId: string): Promise<Session>;
    static getSavedSessions(): SavedSessionInfo[];
    static getSessions(): string[];
    execute(prompt: string): Promise<AgentResult>;
    cancel(): Promise<void>;
    clearHistory(): Promise<void>;
    getHistory(): Promise<ProviderMessage[]>;
    confirmTool(decision: CoreConfirmDecision): Promise<void>;
    subscribe(onEvent: (err: unknown, event?: CoreEvent | null) => void): void;
    unsubscribe(): void;
    getAvailableModels(): Promise<AvailableModel[]>;
    setModel(provider: string, model: string): Promise<void>;
    reloadConfig(): Promise<void>;
    checkLatency(): Promise<LatencyInfo>;
    getAgentMode(): 'plan' | 'build';
    setAgentMode(mode: 'plan' | 'build'): Promise<void>;
    getApprovalMode(): 'read-only' | 'agent' | 'agent-full';
    setApprovalMode(mode: 'read-only' | 'agent' | 'agent-full'): void;
  }
}

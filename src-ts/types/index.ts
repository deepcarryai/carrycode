import type {
  CoreConfirmDecision,
  CoreConfirmationRequest,
  CoreEvent,
  CoreEventType,
  ResponseStage,
  ToolOperation,
  AvailableModel,
  LatencyInfo,
} from 'carrycode-coreapi';

export type {
  CoreConfirmDecision,
  CoreConfirmationRequest,
  CoreEvent,
  CoreEventType,
  ResponseStage,
  ToolOperation,
  AvailableModel,
  LatencyInfo,
};

export interface ToolCallLog {
  operation: ToolOperation;
  toolName: string;
  paramsSummary: string;
  responseSummary?: string;
  status?: 'ok' | 'error';
}

export interface StageSegment {
  stage: ResponseStage;
  title: string;
  content: string;
  tools: ToolCallLog[];
  toolOperation?: ToolOperation;
  isBanner?: boolean;
}

export interface Message {
  question: string;
  segments: StageSegment[];
  startTime?: number;
  duration?: number;
}

export interface WelcomeConfig {
  banner: string[];
  tips?: string[];
  theme?: string;
}

export interface AppConfig {
  welcome?: WelcomeConfig;
  theme?: string;
  providers?: Array<{
    is_active: boolean;
    provider_name: string;
    model_name: string;
    base_url: string;
    api_key: string;
  }>;
  mcp_servers?: Record<
    string,
    | {
      command: string;
      args: string[];
      env: Record<string, string>;
    }
    | {
      url: string;
      headers: Record<string, string>;
    }
  >;
}

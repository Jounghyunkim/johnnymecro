export type StepType = "DETERMINISTIC_ACTION" | "LLM_CONDITION" | "LLM_EXTRACT" | "AI_GOAL_STEP" | "LOOP";
export type ActionKind = "click" | "type" | "press_key" | "scroll" | "wait" | "navigate" | "select";

export interface Fingerprint {
  primaryCss: string;
  xpath: string;
  axRole: string;
  axName: string;
  innerText: string;
  contextText?: string;
  tag?: string;
  attrs?: Record<string, string>;
  boundingBox?: { x: number; y: number; width: number; height: number; viewportWidth: number; viewportHeight: number };
}

export interface Action {
  kind: ActionKind;
  value: string;
  key: string;
  modifiers: number;
  url: string;
  ms: number;
  scrollX: number;
  scrollY: number;
  sensitive: boolean;
}

export interface LoopCfg {
  mode: "count" | "list" | "while";
  count: number;
  listVar: string;
  itemVar: string;
  condition: string;
  maxIterations: number;
}

export interface Step {
  id: string;
  type: StepType;
  name: string;
  enabled: boolean;
  action: Action | null;
  fingerprint: Fingerprint | null;
  timeoutMs: number;
  retries: number;
  llmFallback: boolean;
  onError: "stop" | "continue";
  question: string;
  options: string[];
  branches: Record<string, Step[]>;
  instruction: string;
  schema: unknown;
  outputVar: string;
  goal: string;
  maxSteps: number;
  compileToMacro: boolean;
  loop: LoopCfg | null;
  children: Step[];
}

export interface Scenario {
  id: string;
  name: string;
  description: string;
  createdAt: string;
  updatedAt: string;
  variables: Record<string, unknown>;
  steps: Step[];
}

export interface ScenarioMeta {
  id: string;
  name: string;
  description: string;
  updatedAt: string;
  stepCount: number;
}

export interface Settings {
  debugPort: number;
  chromePath: string;
  userDataDir: string;
  ollamaUrl: string;
  textModel: string;
  visionModel: string;
  visionMode: "never" | "fallback" | "always";
  defaultTimeoutMs: number;
  stepDelayMs: number;
  maxAgentSteps: number;
  confirmSensitive: boolean;
  usePlanner: boolean;
  useValidator: boolean;
  allowedDomains: string;
}

export interface ModelInfo {
  name: string;
  size: number;
  parameterSize: string;
  vision: boolean;
}

export interface PageInfo {
  targetId: string;
  sessionId: string;
  url: string;
  title: string;
  active: boolean;
}

export interface BrowserStatus {
  connected: boolean;
  port?: number;
  pages: PageInfo[];
}

export interface LogEntry {
  level: "info" | "warn" | "error" | "success";
  message: string;
  time: string;
}

export type StepStatus = "running" | "ok" | "healed" | "healing" | "failed" | "skipped";

export interface AgentStep {
  n: number;
  thought: string;
  status: string;
  action: string;
  index: number;
  input: string;
}

export interface AgentTrace {
  goal: string;
  success: boolean;
  message: string;
  steps: Step[];
}

export interface ConfirmReq {
  id: string;
  title: string;
  message: string;
}

/** A stored login. The password itself never reaches the UI. */
export interface CredMeta {
  name: string;
  site: string;
  username: string;
  hasPassword: boolean;
  updatedAt: string;
}

export interface CredPrompt {
  id: string;
  name: string;
  site: string;
  username: string;
  message: string;
}

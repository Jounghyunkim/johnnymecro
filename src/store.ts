import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AgentStep, AgentTrace, CredMeta, CredPrompt, BrowserStatus, ConfirmReq, LogEntry, ModelInfo, Scenario, ScenarioMeta, Settings, Step, StepStatus,
} from "./types";
import { normalizeStep } from "./steps";

interface State {
  settings: Settings | null;
  models: ModelInfo[];
  ollamaError: string;
  browser: BrowserStatus;
  browserBusy: boolean;
  scenarios: ScenarioMeta[];
  current: Scenario | null;
  selectedId: string | null;
  recording: boolean;
  baseSteps: Step[];
  running: boolean;
  runKind: string;
  stepStatus: Record<string, { status: StepStatus; message: string }>;
  logs: LogEntry[];
  vars: Record<string, unknown>;
  confirm: ConfirmReq | null;
  agentSteps: AgentStep[];
  agentTrace: AgentTrace | null;
  creds: CredMeta[];
  credPrompt: CredPrompt | null;

  init: () => Promise<void>;
  log: (level: LogEntry["level"], message: string) => void;
  call: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T | undefined>;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  refreshModels: () => Promise<void>;
  launchBrowser: () => Promise<void>;
  refreshScenarios: () => Promise<void>;
  openScenario: (id: string) => Promise<void>;
  createScenario: (name?: string, steps?: Step[]) => Promise<void>;
  deleteScenario: (id: string) => Promise<void>;
  setSteps: (fn: (steps: Step[]) => Step[]) => void;
  patchScenario: (patch: Partial<Scenario>) => void;
  select: (id: string | null) => void;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<void>;
  run: () => Promise<void>;
  stop: () => Promise<void>;
  runAgent: (goal: string) => Promise<void>;
  answerConfirm: (approve: boolean) => Promise<void>;
  refreshCreds: () => Promise<void>;
  saveCred: (c: { original?: string; name: string; site: string; username: string; password?: string }) => Promise<boolean>;
  deleteCred: (name: string) => Promise<void>;
  answerCredPrompt: (a: { username: string; password: string; save: boolean } | null) => Promise<void>;
}

const nowTime = () => new Date().toTimeString().slice(0, 8);
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let initialized = false;

function normalizeScenario(s: Scenario): Scenario {
  return { ...s, variables: s.variables ?? {}, steps: (s.steps ?? []).map(normalizeStep) };
}

export const useStore = create<State>((set, get) => {
  const scheduleSave = () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flushSave, 500);
  };
  const flushSave = async () => {
    clearTimeout(saveTimer);
    saveTimer = undefined;
    const cur = get().current;
    if (!cur) return;
    try {
      const saved = await invoke<Scenario>("scenario_save", { scenario: cur });
      const now = get().current;
      if (now && now.id === saved.id) set({ current: { ...now, updatedAt: saved.updatedAt, createdAt: saved.createdAt } });
      await get().refreshScenarios();
    } catch (e) {
      get().log("error", `저장 실패: ${e}`);
    }
  };

  return {
    settings: null,
    models: [],
    ollamaError: "",
    browser: { connected: false, pages: [] },
    browserBusy: false,
    scenarios: [],
    current: null,
    selectedId: null,
    recording: false,
    baseSteps: [],
    running: false,
    runKind: "",
    stepStatus: {},
    logs: [],
    vars: {},
    confirm: null,
    agentSteps: [],
    agentTrace: null,
    creds: [],
    credPrompt: null,

    log: (level, message) => set((s) => ({ logs: [...s.logs.slice(-499), { level, message, time: nowTime() }] })),

    call: async <T,>(cmd: string, args?: Record<string, unknown>) => {
      try {
        return await invoke<T>(cmd, args);
      } catch (e) {
        get().log("error", String(e));
        return undefined;
      }
    },

    init: async () => {
      if (initialized) return;
      initialized = true;
      await listen<LogEntry>("log", (e) => set((s) => ({ logs: [...s.logs.slice(-499), e.payload] })));
      await listen<BrowserStatus>("browser://state", (e) => set({ browser: e.payload }));
      await listen<{ recording: boolean }>("record://state", (e) => set({ recording: e.payload.recording }));
      await listen<Step[]>("record://steps", (e) => {
        if (!get().recording) return;
        const rec = e.payload.map(normalizeStep);
        const cur = get().current;
        if (cur) set({ current: { ...cur, steps: [...get().baseSteps, ...rec] } });
      });
      await listen<{ running: boolean; kind: string }>("run://state", (e) => {
        set({ running: e.payload.running, runKind: e.payload.kind });
        if (!e.payload.running) set({ confirm: null, credPrompt: null });
      });
      await listen<{ stepId: string; status: StepStatus; message: string }>("run://step", (e) =>
        set((s) => ({ stepStatus: { ...s.stepStatus, [e.payload.stepId]: { status: e.payload.status, message: e.payload.message } } })),
      );
      await listen<Record<string, unknown>>("run://vars", (e) => set({ vars: e.payload }));
      await listen<ConfirmReq>("run://confirm", (e) => set({ confirm: e.payload }));
      await listen("run://confirm-close", () => set({ confirm: null }));
      await listen<Scenario>("scenario://patched", (e) => {
        const cur = get().current;
        if (cur && cur.id === e.payload.id) set({ current: normalizeScenario(e.payload) });
        void get().refreshScenarios();
      });
      await listen<AgentStep>("agent://step", (e) => set((s) => ({ agentSteps: [...s.agentSteps, e.payload] })));
      await listen<AgentTrace>("agent://trace", (e) => set({ agentTrace: { ...e.payload, steps: e.payload.steps.map(normalizeStep) } }));

      await listen<CredPrompt>("cred://prompt", (e) => set({ credPrompt: e.payload }));
      await listen("cred://prompt-close", () => set({ credPrompt: null }));
      await listen("cred://changed", () => void get().refreshCreds());
      await get().refreshCreds();

      const settings = await get().call<Settings>("get_settings");
      if (settings) set({ settings });
      const b = await get().call<BrowserStatus>("browser_status");
      if (b) set({ browser: b });
      await get().refreshModels();
      await get().refreshScenarios();
      const first = get().scenarios[0];
      if (first) await get().openScenario(first.id);
    },

    saveSettings: async (patch) => {
      const cur = get().settings;
      if (!cur) return;
      const next = { ...cur, ...patch };
      set({ settings: next });
      await get().call("save_settings", { settings: next });
    },

    refreshModels: async () => {
      try {
        const models = await invoke<ModelInfo[]>("ollama_models");
        set({ models, ollamaError: "" });
        const s = get().settings;
        if (s && models.length) {
          const patch: Partial<Settings> = {};
          if (!models.some((m) => m.name === s.textModel)) patch.textModel = models[0].name;
          if (s.visionModel && !models.some((m) => m.name === s.visionModel)) patch.visionModel = "";
          if (!s.visionModel && !("visionModel" in patch)) {
            const v = models.find((m) => m.vision);
            if (v) patch.visionModel = v.name;
          }
          if (Object.keys(patch).length) await get().saveSettings(patch);
        }
      } catch (e) {
        set({ models: [], ollamaError: String(e) });
      }
    },

    launchBrowser: async () => {
      set({ browserBusy: true });
      const b = await get().call<BrowserStatus>("browser_launch");
      if (b) set({ browser: b });
      set({ browserBusy: false });
    },

    refreshScenarios: async () => {
      const list = await get().call<ScenarioMeta[]>("scenario_list");
      if (list) set({ scenarios: list });
    },

    openScenario: async (id) => {
      if (get().recording || get().running) return;
      if (saveTimer) await flushSave();
      const s = await get().call<Scenario>("scenario_load", { id });
      if (s) set({ current: normalizeScenario(s), selectedId: null, stepStatus: {} });
    },

    createScenario: async (name, steps) => {
      if (get().recording || get().running) return;
      if (saveTimer) await flushSave();
      const draft: Scenario = {
        id: "",
        name: name || `새 시나리오 ${get().scenarios.length + 1}`,
        description: "",
        createdAt: "",
        updatedAt: "",
        variables: {},
        steps: steps ?? [],
      };
      const saved = await get().call<Scenario>("scenario_save", { scenario: draft });
      if (saved) {
        set({ current: normalizeScenario(saved), selectedId: null, stepStatus: {} });
        await get().refreshScenarios();
      }
    },

    deleteScenario: async (id) => {
      if (get().recording || get().running) return;
      clearTimeout(saveTimer);
      saveTimer = undefined;
      await get().call("scenario_delete", { id });
      if (get().current?.id === id) set({ current: null, selectedId: null });
      await get().refreshScenarios();
      const first = get().scenarios[0];
      if (!get().current && first) await get().openScenario(first.id);
    },

    setSteps: (fn) => {
      const cur = get().current;
      if (!cur || get().running) return;
      set({ current: { ...cur, steps: fn(cur.steps) } });
      scheduleSave();
    },

    patchScenario: (patch) => {
      const cur = get().current;
      if (!cur) return;
      set({ current: { ...cur, ...patch } });
      scheduleSave();
    },

    select: (id) => set({ selectedId: id }),

    startRecording: async () => {
      if (!get().current) await get().createScenario();
      const cur = get().current;
      if (!cur) return;
      if (!get().browser.connected) {
        await get().launchBrowser();
        if (!get().browser.connected) return;
      }
      set({ baseSteps: cur.steps, recording: true });
      try {
        await invoke("record_start");
      } catch (e) {
        set({ recording: false });
        get().log("error", String(e));
      }
    },

    stopRecording: async () => {
      const rec = await get().call<Step[]>("record_stop");
      const cur = get().current;
      if (cur && rec) set({ current: { ...cur, steps: [...get().baseSteps, ...rec.map(normalizeStep)] } });
      set({ recording: false, baseSteps: [] });
      await flushSave();
    },

    run: async () => {
      const cur = get().current;
      if (!cur || get().running || get().recording) return;
      if (!get().browser.connected) {
        await get().launchBrowser();
        if (!get().browser.connected) return;
      }
      await flushSave();
      set({ stepStatus: {}, vars: {} });
      await get().call("run_scenario", { scenario: get().current });
    },

    stop: async () => {
      await get().call("run_stop");
    },

    runAgent: async (goal) => {
      if (get().running || get().recording) return;
      if (!get().browser.connected) {
        await get().launchBrowser();
        if (!get().browser.connected) return;
      }
      set({ agentSteps: [], agentTrace: null });
      await get().call("agent_run", { goal });
    },

    refreshCreds: async () => {
      const list = await get().call<CredMeta[]>("cred_list");
      if (list) set({ creds: list });
    },

    // The password only travels UI -> backend; it is never kept in this store or sent back.
    saveCred: async (c) => {
      try {
        await invoke("cred_save", { original: c.original ?? null, name: c.name, site: c.site, username: c.username, password: c.password || null });
        await get().refreshCreds();
        return true;
      } catch (e) {
        get().log("error", String(e));
        return false;
      }
    },

    deleteCred: async (name) => {
      await get().call("cred_delete", { name });
      await get().refreshCreds();
    },

    answerCredPrompt: async (a) => {
      const p = get().credPrompt;
      if (!p) return;
      set({ credPrompt: null });
      await get().call("respond_cred_prompt", { id: p.id, username: a?.username ?? "", password: a?.password ?? "", save: a?.save ?? false, cancel: !a });
    },

    answerConfirm: async (approve) => {
      const c = get().confirm;
      if (!c) return;
      set({ confirm: null });
      await get().call("respond_confirm", { id: c.id, approve });
    },
  };
});

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import kiritoIconUrl from "./assets/kirito-icon.png";
import compatibilityRules from "./compatibility-rules.json";
import "./styles.css";

type BottleSettings = {
  d3dMetal: boolean;
  dxvk: boolean;
  msync: boolean;
  retina: boolean;
  vsync: boolean;
};

type AppSettings = {
  engineExecutable: string | null;
  d3dMetal: boolean | null;
  dxvk: boolean | null;
  msync: boolean | null;
  retina: boolean | null;
  vsync: boolean | null;
};

type AppSettingsRecommendation = {
  settings: AppSettings;
  confidence: "High" | "Medium" | "Low";
  title: string;
  rationale: string[];
  warnings: string[];
};

type CompatibilityRuleSettings = {
  d3dMetal?: boolean;
  dxvk?: boolean;
  msync?: boolean;
  retina?: boolean;
  vsync?: boolean;
};

type CompatibilityRule = {
  appId?: number;
  name?: string;
  title?: string;
  confidence?: AppSettingsRecommendation["confidence"];
  settings?: CompatibilityRuleSettings;
  rationale?: string[];
  warnings?: string[];
};

type CompatibilityRules = {
  launcherRiskTokens: string[];
  antiCheatRiskTokens: string[];
  appRules: CompatibilityRule[];
};

const localCompatibilityRules = compatibilityRules as CompatibilityRules;

type InstalledApp = {
  id: string;
  name: string;
  command: string;
};

type Bottle = {
  id: string;
  name: string;
  path: string;
  windowsVersion: string;
  settings: BottleSettings;
  appSettings: Record<string, AppSettings>;
  apps: InstalledApp[];
};

type WineEngine = {
  displayName: string;
  executable: string;
  d3dMetalCapable?: boolean;
};

type SteamStatus = {
  installed: boolean;
  running: boolean;
  bottleId: string | null;
  bottlePath: string | null;
  games: InstalledSteamGame[];
};

type SteamGame = {
  appId: number;
  name: string;
  image: string | null;
  price: string | null;
  storeUrl: string;
};

type InstalledSteamGame = {
  appId: number;
  name: string;
  installDir: string | null;
  image: string;
};

type ProtonRating = {
  appId: number;
  tier: string;
  confidence: string | null;
  score: number | null;
  total: number | null;
  sourceUrl: string;
};

type AntiCheatReport = {
  matchedName: string;
  name: string;
  status: string;
  antiCheats: string[];
  notes: string | null;
  updated: string | null;
  sourceUrl: string;
};

type RuntimeStatus = {
  steamRunning: boolean;
  appIds: string[];
  steamAppIds: number[];
};

type SteamDiagnostic = {
  title: string;
  detail: string;
  level: "warn" | "info";
};

type LaunchFailure = {
  appId: number;
  gameName: string;
  title: string;
  detail: string;
  action: "clear-cache" | "stop-steam" | "settings" | "retry";
  actionLabel: string;
};

type ControllerDevice = {
  name: string;
  transport: string;
  vendor: string | null;
};

type ControllerDiagnostics = {
  checkedAt: number;
  devices: ControllerDevice[];
};

type InstallProgress = {
  appName: string;
  message: string;
  stepIndex?: number | null;
  stepTotal?: number | null;
  percent?: number | null;
};

type View = "install" | "library" | "settings";
type ThemeMode = "auto" | "dark" | "light";

type AppSettingsTarget = {
  bottleId: string;
  appKey: string;
  name: string;
  subtitle: string;
};

type RecentApp = {
  name: string;
  subtitle: string;
  iconUrl?: string;
  action: "launch-steam" | "run-app" | "run-steam-game";
  bottleId?: string;
  appId?: string;
  steamAppId?: number;
};

type CatalogApp = {
  id: string;
  name: string;
  vendor: string;
  summary: string;
  infoSource: string;
  installSource: string;
  reportSource: "protondb" | "winehq" | "community";
  profileSignal: string;
  demand: string;
  recommendedSettings: string[];
  knownIssues: string[];
  wantedFeatures: string[];
  installPlan?: InstallPlanStep[];
  steamAppId?: number;
  badge: string;
  iconUrl: string;
};

type InstallPlanStep = {
  title: string;
  detail: string;
  kind: "download" | "bottle" | "dependency" | "installer";
};

type AppState = {
  bottles: Bottle[];
  engines: WineEngine[];
  steam: SteamStatus;
  bedrock: { runtimeReady: boolean; installed: boolean };
  steamResults: SteamGame[];
  protonRatings: Record<number, ProtonRating | null>;
  antiCheatReports: Record<string, AntiCheatReport | null>;
  controllerDiagnostics: ControllerDiagnostics | null;
  launchFailure: LaunchFailure | null;
  runtime: RuntimeStatus;
  steamQuery: string;
  installQuery: string;
  selectedBottleId: string | null;
  detailCatalogId: string | null;
  appSettingsTarget: AppSettingsTarget | null;
  lastUsedApp: RecentApp | null;
  theme: ThemeMode;
  view: View;
  status: string;
  installProgress: InstallProgress | null;
  busy: boolean;
  searching: boolean;
};

const state: AppState = {
  bedrock: { runtimeReady: false, installed: false },
  bottles: [],
  engines: [],
  steam: { installed: false, running: false, bottleId: null, bottlePath: null, games: [] },
  steamResults: [],
  protonRatings: {},
  antiCheatReports: {},
  controllerDiagnostics: null,
  launchFailure: null,
  runtime: { steamRunning: false, appIds: [], steamAppIds: [] },
  steamQuery: "",
  installQuery: "",
  selectedBottleId: null,
  detailCatalogId: null,
  appSettingsTarget: null,
  lastUsedApp: loadLastUsedApp(),
  theme: loadTheme(),
  view: "install",
  status: "",
  installProgress: null,
  busy: false,
  searching: false
};

const app = document.querySelector<HTMLDivElement>("#app");
const STEAM_STATUS_POLL_MS = 2500;
let steamStatusPollInFlight = false;
let controllerDiagnosticsInFlight = false;
let engineEnsureInFlight = false;
let searchTimer: number | null = null;
let installSearchTimer: number | null = null;
const launchingSteamAppIds = new Set<number>();

const catalogApps: CatalogApp[] = [
  {
    id: "minecraft-bedrock",
    name: "Minecraft Bedrock",
    vendor: "Mojang / Microsoft",
    summary: "Play your Windows PC edition using an experimental community runtime. Requires Microsoft sign-in and ownership of Minecraft for Windows.",
    infoSource: "Bedrock GDK community project",
    installSource: "Microsoft Store via Xodus; pinned community runtime",
    reportSource: "community",
    profileSignal: "Experimental — gameplay verification pending",
    demand: "Windows edition on Apple silicon",
    recommendedSettings: ["Dedicated Bedrock bottle", "D3DMetal", "Keep the supplied GDK runtime"],
    knownIssues: ["Experimental third-party runtime, not an official macOS release", "The runtime provider has not published all modified source code", "Microsoft sign-in is required for your PC purchase", "Xbox sign-in may also be required in game", "New game versions may break compatibility"],
    wantedFeatures: [],
    installPlan: [
      { title: "Prepare Bedrock runtime", detail: "Download and checksum-verify the pinned community Wine/GDK runtime", kind: "download" },
      { title: "Sign in to Microsoft", detail: "Xodus opens Microsoft's sign-in page to access your existing PC purchase", kind: "installer" },
      { title: "Download Bedrock", detail: "Retrieve and prepare your licensed game files from Microsoft", kind: "download" },
      { title: "Create Bedrock bottle", detail: "Configure D3DMetal and GameInput in an isolated bottle", kind: "bottle" }
    ],
    badge: "MC",
    iconUrl: favicon("minecraft.net")
  },
  {
    id: "steam",
    name: "Steam",
    vendor: "Valve",
    summary: "A Windows game storefront and launcher. kirito can install it, detect its library, and open installed Steam games directly.",
    infoSource: "kirito catalog + Steam",
    installSource: "Valve Steam installer",
    reportSource: "winehq",
    profileSignal: "Public report signal",
    demand: "#1 ranked app, top one-click install",
    recommendedSettings: ["Use the Steam bottle", "Windows 10 profile", "D3DMetal on", "DXVK off by default", "MSync on", "Retina off", "VSync on"],
    knownIssues: ["Some games need per-game graphics settings", "Launcher may stay open after game exit", "Overlay and purchase windows can be inconsistent"],
    wantedFeatures: ["Per-game defaults", "Repair Steam web cache", "Kill stuck launcher processes", "Show anti-cheat warnings"],
    installPlan: [
      { title: "Download Steam installer", detail: "Valve SteamSetup.exe", kind: "download" },
      { title: "Create Steam bottle", detail: "Windows 10 compatibility profile", kind: "bottle" },
      { title: "Install CJK font support", detail: "Source Han Sans / CJK font package when helper support is available", kind: "dependency" },
      { title: "Install Wine browser components", detail: "Wine Mono and HTML engine support where available", kind: "dependency" },
      { title: "Install Visual C++ runtimes", detail: "Latest VC++ redistributables for x86 and x64", kind: "dependency" },
      { title: "Install legacy text libraries", detail: "RichEdit 4.1 and Microsoft Line Services", kind: "dependency" },
      { title: "Run Steam setup", detail: "Launch Steam and let it update normally", kind: "installer" }
    ],
    badge: "ST",
    iconUrl: favicon("steampowered.com")
  },
  {
    id: "office",
    name: "Microsoft Office 2010",
    vendor: "Microsoft",
    summary: "Classic Windows Office release often used as a productivity profile target.",
    infoSource: "kirito catalog",
    installSource: "User-provided Microsoft installer",
    reportSource: "winehq",
    profileSignal: "Legacy productivity target",
    demand: "Common non-game reason to use Wine on Mac",
    recommendedSettings: ["Use a separate productivity bottle", "D3DMetal off", "DXVK off", "Retina off if text/layout glitches", "Keep default sync"],
    knownIssues: ["Activation and installers vary by media", "Some integrations expect Windows services", "High-DPI rendering can be uneven"],
    wantedFeatures: ["Office install recipe", "Activation notes", "File association helpers", "Font smoothing controls"],
    badge: "OF",
    iconUrl: favicon("microsoft.com")
  },
  {
    id: "quicken",
    name: "Quicken Classic",
    vendor: "Quicken",
    summary: "Windows personal finance software commonly installed when a native Mac workflow is not enough.",
    infoSource: "kirito catalog",
    installSource: "User-provided Quicken installer",
    reportSource: "winehq",
    profileSignal: "Known productivity use case",
    demand: "Common finance/legacy app target",
    recommendedSettings: ["Use a separate finance bottle", "D3DMetal off", "DXVK off", "Retina off", "Back up data before updates"],
    knownIssues: ["Online banking flows can depend on embedded browser components", "Updates may change login or sync behavior", "Printer/PDF workflows can need extra setup"],
    wantedFeatures: ["Data backup prompt", "Safe update mode", "PDF/print setup", "Per-version notes"],
    badge: "Q",
    iconUrl: favicon("quicken.com")
  },
  {
    id: "battlenet",
    name: "Battle.net",
    vendor: "Blizzard",
    summary: "Blizzard game launcher for Windows games and account-managed installs.",
    infoSource: "Public usage stats",
    installSource: "Official Blizzard installer",
    reportSource: "winehq",
    profileSignal: "Public report signal",
    demand: "#2 ranked launcher",
    recommendedSettings: ["Use a launcher bottle", "D3DMetal on for modern games", "MSync on", "Retina off", "Keep a fallback profile with Wine graphics"],
    knownIssues: ["Launcher updates can regress game startup", "Some games rely on anti-cheat or account overlays", "A running launcher does not always mean the game window opened"],
    wantedFeatures: ["Launcher repair", "Per-game profiles under the launcher", "Anti-cheat warning", "Stuck-launch detection"],
    badge: "BN",
    iconUrl: favicon("battle.net")
  },
  {
    id: "epic",
    name: "Epic Games Launcher",
    vendor: "Epic Games",
    summary: "Windows game storefront and launcher for Epic libraries.",
    infoSource: "Public usage stats",
    installSource: "Official Epic installer",
    reportSource: "winehq",
    profileSignal: "Public report signal",
    demand: "#3 ranked launcher",
    recommendedSettings: ["Use a launcher bottle", "D3DMetal on for supported games", "MSync on", "Retina off", "Use Wine graphics fallback for launcher issues"],
    knownIssues: ["Web login and launcher rendering can be fragile", "Some games need separate backend choices", "Online services and anti-cheat vary per game"],
    wantedFeatures: ["Trusted installer manifest", "Launcher cache repair", "Per-game launch args", "Anti-cheat labels"],
    badge: "EG",
    iconUrl: favicon("epicgames.com")
  },
  {
    id: "gog",
    name: "GOG Galaxy",
    vendor: "GOG",
    summary: "GOG library manager and optional launcher for DRM-free game installs.",
    infoSource: "kirito catalog",
    installSource: "Official GOG installer",
    reportSource: "winehq",
    profileSignal: "Likely launcher target",
    demand: "Common DRM-free library use case",
    recommendedSettings: ["Use a launcher bottle", "D3DMetal on for games", "MSync on", "Retina off", "Prefer direct game shortcuts when possible"],
    knownIssues: ["Launcher integrations can be less important than direct game launch", "Game settings vary widely", "Cloud save/login features can be inconsistent"],
    wantedFeatures: ["Direct EXE discovery", "Game shortcut import", "Offline installer support", "Per-game notes"],
    badge: "GG",
    iconUrl: favicon("gog.com")
  },
  {
    id: "rockstar",
    name: "Rockstar Games Launcher",
    vendor: "Rockstar",
    summary: "Rockstar account launcher required by several Rockstar Windows games.",
    infoSource: "kirito catalog",
    installSource: "Official Rockstar installer",
    reportSource: "winehq",
    profileSignal: "High-demand launcher path",
    demand: "Needed for GTA and Rockstar libraries",
    recommendedSettings: ["Use with game-specific profile", "D3DMetal on for modern titles", "MSync on", "Retina off", "Expect launcher dependency"],
    knownIssues: ["Login/update flows can block game launch", "Online modes may depend on anti-cheat or services", "Launcher may stay resident"],
    wantedFeatures: ["Launcher health check", "Per-game profile mapping", "Online-mode warning", "Log viewer"],
    badge: "R",
    iconUrl: favicon("rockstargames.com")
  },
  {
    id: "ubisoft",
    name: "Ubisoft Connect",
    vendor: "Ubisoft",
    summary: "Ubisoft launcher for Windows game installs, account login, and game updates.",
    infoSource: "Public usage stats",
    installSource: "Official Ubisoft installer",
    reportSource: "winehq",
    profileSignal: "Public report signal",
    demand: "#8 ranked launcher",
    recommendedSettings: ["Use a launcher bottle", "D3DMetal on for newer games", "MSync on", "Retina off", "Wine graphics fallback for launcher rendering"],
    knownIssues: ["Account/login windows can be fragile", "Some games require extra launcher handoff", "Online services vary per game"],
    wantedFeatures: ["Login repair steps", "Game handoff detection", "Known-good profile presets", "Anti-cheat labels"],
    badge: "UC",
    iconUrl: favicon("ubisoft.com")
  },
  {
    id: "notepadpp",
    name: "Notepad++",
    vendor: "Don Ho",
    summary: "Lightweight Windows text editor and a good simple app install test.",
    infoSource: "kirito catalog",
    installSource: "Official Notepad++ installer",
    reportSource: "winehq",
    profileSignal: "Simple utility target",
    demand: "Useful smoke test and utility",
    recommendedSettings: ["Use a utility bottle", "D3DMetal off", "DXVK off", "Default sync", "Retina optional"],
    knownIssues: ["File associations are the main integration gap", "Plugins may need extra runtimes", "Fonts can look different than native macOS"],
    wantedFeatures: ["File association setup", "Open-with integration", "Plugin dependency notes", "Font smoothing control"],
    badge: "N+",
    iconUrl: favicon("notepad-plus-plus.org")
  },
  {
    id: "vortex",
    name: "Vortex",
    vendor: "Nexus Mods",
    summary: "Windows mod manager for game libraries that need external mod tooling.",
    infoSource: "kirito catalog",
    installSource: "Official Nexus Mods installer",
    reportSource: "winehq",
    profileSignal: "Advanced utility target",
    demand: "Modding workflow need",
    recommendedSettings: ["Use the same bottle as the target game when possible", "D3DMetal follows the game", "MSync follows the game", "Retina off"],
    knownIssues: ["Needs access to game folders", "Symlinks and path mapping can confuse tools", "External helpers may need runtimes"],
    wantedFeatures: ["Bottle folder picker", "Game path linking", "Mod manager profiles", "Dependency recipes"],
    badge: "VX",
    iconUrl: favicon("nexusmods.com")
  },
  {
    id: "cs2",
    name: "Counter-Strike 2",
    vendor: "Valve",
    summary: "Needs CrossOver/GPTK-class D3DMetal for playable Mac performance. Kirito applies the CrossOver parity profile on launch.",
    infoSource: "Steam Store + CrossOver community",
    installSource: "Steam client",
    reportSource: "protondb",
    profileSignal: "CrossOver parity profile",
    demand: "Top Mac Wine FPS target",
    recommendedSettings: ["D3DMetal on", "DXVK off", "MSync on", "Retina off", "VSync on", "CrossOver or GPTK engine"],
    knownIssues: ["Stock Wine Stable is not enough", "Needs D3DMetal-capable engine", "Shader compile stutter on first runs"],
    wantedFeatures: ["Auto VideoMemorySize", "D3DMetal engine detection", "CS2 launch options"],
    steamAppId: 730,
    badge: "CS2",
    iconUrl: favicon("counter-strike.net")
  },
  {
    id: "witcher3",
    name: "The Witcher 3: Wild Hunt",
    vendor: "CD Projekt",
    summary: "Windows Steam game used here as a profile reference for Steam installs.",
    infoSource: "Steam Store",
    installSource: "Steam client",
    reportSource: "protondb",
    profileSignal: "Steam profile needed",
    demand: "Popular Steam game target",
    recommendedSettings: ["Run through Steam", "D3DMetal on", "DXVK off initially", "MSync on", "Retina off", "VSync on"],
    knownIssues: ["Renderer choice can affect flicker/performance", "Launcher overlays may appear before the game", "Modded installs need separate testing"],
    wantedFeatures: ["Auto profile from Steam app id", "Renderer fallback button", "Launch args", "Modded install notes"],
    steamAppId: 292030,
    badge: "W3",
    iconUrl: favicon("thewitcher.com")
  },
  {
    id: "gta5",
    name: "Grand Theft Auto V",
    vendor: "Rockstar",
    summary: "Windows Steam game used here as a profile reference for Steam installs.",
    infoSource: "Steam Store",
    installSource: "Steam client + Rockstar launcher",
    reportSource: "protondb",
    profileSignal: "Launcher-dependent game",
    demand: "#6 ranked game path",
    recommendedSettings: ["Run through Steam", "D3DMetal on", "MSync on", "Retina off", "Expect Rockstar launcher"],
    knownIssues: ["Online mode and launcher behavior can change", "Anti-cheat/service requirements may block some flows", "The launcher may keep running after the game closes"],
    wantedFeatures: ["Launcher chain status", "Online-mode warning", "Per-game process cleanup", "Known-good profile history"],
    steamAppId: 271590,
    badge: "V",
    iconUrl: favicon("rockstargames.com")
  }
];

if (!app) {
  throw new Error("Missing #app");
}

async function apiInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if ("__TAURI_INTERNALS__" in window) {
    return tauriInvoke<T>(command, args);
  }

  return mockInvoke<T>(command, args);
}

async function setupInstallProgress() {
  if (!("__TAURI_INTERNALS__" in window)) return;

  await listen<string>("bedrock-status", (event) => {
    state.status = event.payload;
    render();
  });

  await listen<InstallProgress>("install-progress", (event) => {
    const isRuntimeSetup = event.payload.appName === "Wine" || event.payload.appName === "D3DMetal";
    if (!state.busy && !isRuntimeSetup) return;
    if (isRuntimeSetup && (state.engines.length === 0 || !state.engines.some((engine) => engine.d3dMetalCapable))) {
      state.busy = true;
    }
    state.installProgress = event.payload;
    state.status = event.payload.message || `Installing ${event.payload.appName}`;
    if (isRuntimeSetup && (event.payload.percent ?? 0) >= 100) {
      state.busy = false;
      void refresh();
      return;
    }
    render();
  });
}

async function mockInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const steamBottle: Bottle = {
    id: "steam-demo",
    name: "Steam",
    path: "~/Library/Application Support/kirito/Bottles/steam",
    windowsVersion: "Windows 11",
    settings: { d3dMetal: true, dxvk: false, msync: true, retina: false, vsync: true },
    appSettings: {},
    apps: [
      { id: "steam-app", name: "Steam", command: "C:\\Program Files (x86)\\Steam\\steam.exe" },
      { id: "notepad-demo", name: "Notepad++", command: "C:\\Program Files\\Notepad++\\notepad++.exe" }
    ]
  };
  const demoGames: InstalledSteamGame[] = [
    {
      appId: 620,
      name: "Portal 2",
      installDir: "Portal 2",
      image: "https://cdn.cloudflare.steamstatic.com/steam/apps/620/header.jpg"
    },
    {
      appId: 413150,
      name: "Stardew Valley",
      installDir: "Stardew Valley",
      image: "https://cdn.cloudflare.steamstatic.com/steam/apps/413150/header.jpg"
    }
  ];

  const mocks: Record<string, unknown> = {
    bedrock_status: { runtimeReady: false, installed: false },
    list_bottles: [steamBottle],
    list_engines: [{ displayName: "App bundle", executable: "/Applications/Runtime.app/.../wine", d3dMetalCapable: true }],
    ensure_engine: [{ displayName: "Wine Stable (kirito)", executable: "~/Library/Application Support/app.kirito.desktop/Engines/Wine Stable.app/Contents/Resources/wine/bin/wine", d3dMetalCapable: false }],
    steam_status: { installed: true, running: false, bottleId: "steam-demo", bottlePath: steamBottle.path, games: demoGames },
    runtime_status: { steamRunning: false, appIds: [], steamAppIds: [] },
    create_bottle: steamBottle,
    install_steam: steamBottle,
    launch_steam: undefined,
    apply_steam_compatibility_mode: steamBottle,
    set_bottle_vsync: steamBottle,
    set_app_settings: steamBottle,
    scan_steam_library: demoGames,
    open_steam_game: undefined,
    run_steam_game: undefined,
    proton_rating: { appId: 620, tier: "platinum", confidence: "strong", score: 0.88, total: 323, sourceUrl: "https://www.protondb.com/app/620" },
    run_wine_tool: undefined,
    run_app: undefined,
    install_app: undefined,
    open_path: undefined
  };

  if (command === "search_steam") {
    const query = String(args?.query ?? "Portal");
    return [
      {
        appId: 620,
        name: query.toLowerCase().includes("hades") ? "Hades II" : "Portal 2",
        image: "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/capsule_231x87.jpg",
        price: "$9.99",
        storeUrl: "https://store.steampowered.com/app/620"
      },
      {
        appId: 400,
        name: query.toLowerCase().includes("hades") ? "Hades" : "Portal",
        image: "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/400/capsule_231x87.jpg",
        price: "$9.99",
        storeUrl: "https://store.steampowered.com/app/400"
      }
    ] as T;
  }

  return mocks[command] as T;
}

async function refresh() {
  const [bottles, engines, steam, bedrock] = await Promise.all([
    apiInvoke<Bottle[]>("list_bottles"),
    apiInvoke<WineEngine[]>("list_engines"),
    apiInvoke<SteamStatus>("steam_status"),
    apiInvoke<AppState["bedrock"]>("bedrock_status")
  ]);
  const runtime = await apiInvoke<RuntimeStatus>("runtime_status", { knownSteamGames: steam.games });

  state.bedrock = bedrock;
  state.bottles = bottles;
  state.engines = engines;
  state.steam = { ...steam, running: runtime.steamRunning };
  state.runtime = runtime;
  reconcileLastUsedApp();
  if (!engineEnsureInFlight) {
    state.busy = false;
    state.status = "";
    state.installProgress = null;
  }

  if (!state.selectedBottleId && bottles.length > 0) {
    state.selectedBottleId = bottles[0].id;
  }
  if (state.selectedBottleId && !bottles.some((bottle) => bottle.id === state.selectedBottleId)) {
    state.selectedBottleId = bottles[0]?.id ?? null;
  }
  render();
  void loadProtonRatings([
    ...catalogApps.map((item) => item.steamAppId).filter((appId): appId is number => Boolean(appId)),
    ...steam.games.map((game) => game.appId)
  ]);
  void loadAntiCheatReports(steam.games);
  if (steam.installed) {
    void loadControllerDiagnostics();
  }
  if (engines.length === 0) {
    void ensureEngineInstalled();
  }
}

async function ensureEngineInstalled() {
  if (engineEnsureInFlight || state.engines.length > 0) return;
  if (!("__TAURI_INTERNALS__" in window)) return;

  engineEnsureInFlight = true;
  state.busy = true;
  state.status = "Installing Wine engine…";
  state.installProgress = {
    appName: "Wine",
    message: "Preparing Wine Stable…",
    stepIndex: 1,
    stepTotal: 3,
    percent: 5
  };
  render();

  try {
    const engines = await apiInvoke<WineEngine[]>("ensure_engine");
    state.engines = engines;
    state.status = engines.length ? "Wine engine ready" : "Wine engine install finished, but no engine was found";
    state.installProgress = null;
  } catch (error) {
    state.status = error instanceof Error ? error.message : String(error);
    state.installProgress = null;
  } finally {
    engineEnsureInFlight = false;
    state.busy = false;
    render();
  }
}

async function loadControllerDiagnostics() {
  if (controllerDiagnosticsInFlight) return;
  controllerDiagnosticsInFlight = true;
  try {
    state.controllerDiagnostics = await apiInvoke<ControllerDiagnostics>("controller_diagnostics");
    render();
  } catch {
    state.controllerDiagnostics = null;
  } finally {
    controllerDiagnosticsInFlight = false;
  }
}

async function loadAntiCheatReports(games: InstalledSteamGame[]) {
  const names = games
    .map((game) => game.name)
    .filter((name) => !state.antiCheatReports[normalizeAppName(name)]);
  if (names.length === 0) return;

  try {
    const reports = await apiInvoke<AntiCheatReport[]>("anti_cheat_reports", { names });
    for (const name of names) {
      state.antiCheatReports[normalizeAppName(name)] = null;
    }
    for (const report of reports) {
      state.antiCheatReports[normalizeAppName(report.matchedName)] = report;
    }
    render();
  } catch {
    for (const name of names) {
      state.antiCheatReports[normalizeAppName(name)] = null;
    }
  }
}

async function refreshSteamStatus({ quiet = true } = {}) {
  if (steamStatusPollInFlight) return;
  steamStatusPollInFlight = true;
  try {
    const runtime = await apiInvoke<RuntimeStatus>("runtime_status", { knownSteamGames: state.steam.games });
    const steamRunningChanged = state.steam.running !== runtime.steamRunning;
    const changed = steamRunningChanged || runtimeStatusChanged(state.runtime, runtime);
    state.steam = { ...state.steam, running: runtime.steamRunning };
    state.runtime = runtime;
    let clearedStartingState = false;
    for (const appId of runtime.steamAppIds) {
      clearedStartingState = launchingSteamAppIds.delete(appId) || clearedStartingState;
    }
    const hadLaunchFailure = Boolean(state.launchFailure);
    if (state.launchFailure && runtime.steamAppIds.includes(state.launchFailure.appId)) {
      state.launchFailure = null;
    }
    if (changed || clearedStartingState || hadLaunchFailure !== Boolean(state.launchFailure)) {
      render();
    }
  } catch (error) {
    if (!quiet) {
      state.status = error instanceof Error ? error.message : String(error);
      state.busy = false;
      render();
    }
  } finally {
    steamStatusPollInFlight = false;
  }
}

function runtimeStatusChanged(previous: RuntimeStatus, next: RuntimeStatus) {
  return previous.steamRunning !== next.steamRunning
    || !samePrimitiveSet(previous.appIds, next.appIds)
    || !samePrimitiveSet(previous.steamAppIds, next.steamAppIds);
}

function samePrimitiveSet<T extends string | number>(previous: T[], next: T[]) {
  if (previous.length !== next.length) return false;
  const nextSet = new Set(next);
  return previous.every((item) => nextSet.has(item));
}

function selectedBottle() {
  return state.bottles.find((bottle) => bottle.id === state.selectedBottleId) ?? null;
}

async function runCommand<T>(message: string, action: () => Promise<T>) {
  state.status = message;
  state.busy = true;
  render();
  try {
    await action();
    await refresh();
  } catch (error) {
    state.status = error instanceof Error ? error.message : String(error);
    state.installProgress = null;
    state.busy = false;
    render();
  }
}

async function runLaunchCommand<T>(message: string, action: () => Promise<T>) {
  try {
    await action();
    window.setTimeout(() => void refreshSteamStatus(), 900);
  } catch (error) {
    state.status = error instanceof Error ? error.message : String(error);
    state.installProgress = null;
    state.busy = false;
    render();
  }
}

async function runSteamGameLaunch(appId: number, message = "Starting game") {
  launchingSteamAppIds.add(appId);
  state.launchFailure = null;
  render();
  try {
    await runLaunchCommand(message, () => apiInvoke("run_steam_game", { appId }));
    window.setTimeout(() => void evaluateSteamLaunch(appId), 7000);
    window.setTimeout(() => {
      launchingSteamAppIds.delete(appId);
      render();
    }, 15000);
  } catch {
    launchingSteamAppIds.delete(appId);
    render();
  }
}

async function evaluateSteamLaunch(appId: number) {
  await refreshSteamStatus();
  if (steamGameIsRunning(appId)) return;
  launchingSteamAppIds.delete(appId);
  state.launchFailure = explainSteamLaunchFailure(appId);
  render();
}

function systemStatus() {
  if (state.busy || state.status) return state.status;
  if (state.engines.length === 0) return "Installing Wine engine…";
  if (state.steam.running) return "Steam is open";
  if (state.steam.installed) return `${libraryItemCount()} apps installed`;
  return state.bottles.length ? `${libraryItemCount()} apps installed` : "No apps installed";
}

function topbarStatus() {
  if (!(state.busy || state.status)) return "";

  const progress = state.installProgress;
  const percent = progress?.percent ?? null;
  const stepLabel = progress?.stepIndex && progress.stepTotal
    ? `${progress.stepIndex}/${progress.stepTotal}`
    : "";

  return `
    <div class="status">
      <span>${escapeHtml(systemStatus())}</span>
      ${progress ? `
        <div class="status-progress" aria-label="Install progress">
          <div class="status-progress-meta">
            <small>${escapeHtml(progress.appName)}</small>
            <small>${escapeHtml(stepLabel)}</small>
          </div>
          <div class="status-progress-track">
            <span style="width: ${Math.max(0, Math.min(100, percent ?? 8))}%"></span>
          </div>
        </div>
      ` : ""}
    </div>
  `;
}

function sidebarInstallIndicator() {
  const progress = state.installProgress;
  if (!state.busy || !progress) return "";

  const percent = Math.max(0, Math.min(100, progress.percent ?? 8));
  const stepLabel = progress.stepIndex && progress.stepTotal
    ? `${progress.stepIndex}/${progress.stepTotal}`
    : "Installing";

  return `
    <section class="install-detection" aria-label="Install activity">
      <div class="install-ring" style="--progress: ${percent}%">
        <span></span>
      </div>
      <div>
        <strong>${escapeHtml(stepLabel)}</strong>
        <small>${escapeHtml(progress.message || `Installing ${progress.appName}`)}</small>
      </div>
    </section>
  `;
}

function sidebarActivity() {
  const running = runningActivity();
  if (running) {
    return `
      <section class="sidebar-activity">
        <span>Currently running</span>
        <button ${running.attribute}>
          ${activityIcon(running.iconUrl, running.fallback)}
          <div>
            <strong>${escapeHtml(running.name)}</strong>
          </div>
        </button>
      </section>
    `;
  }

  if (!state.lastUsedApp) return "";

  return `
      <section class="sidebar-activity">
        <span>Last used</span>
        <button data-open-last-used>
          ${activityIcon(state.lastUsedApp.iconUrl, "K")}
          <div>
            <strong>${escapeHtml(state.lastUsedApp.name)}</strong>
            <small>${escapeHtml(state.lastUsedApp.subtitle)}</small>
          </div>
        </button>
      </section>
  `;
}

function runningActivity() {
  const runningApp = launcherItems().find((item) =>
    normalizeAppName(item.name) === "steam" ? state.steam.running : appIsRunning(item.id)
  );
  if (runningApp) {
    const catalogItem = catalogAppForName(runningApp.name);
    const isSteam = normalizeAppName(runningApp.name) === "steam";
    return {
      name: runningApp.name,
      iconUrl: catalogItem?.iconUrl,
      fallback: catalogItem?.badge ?? "K",
      attribute: isSteam ? "data-launch-steam" : `data-run-app="${runningApp.id}" data-bottle="${runningApp.bottleId}"`
    };
  }

  const runningGame = state.steam.games.find((game) => steamGameIsRunning(game.appId));
  if (runningGame) {
    return {
      name: runningGame.name,
      iconUrl: runningGame.image,
      fallback: "ST",
      attribute: `data-run-steam-game="${runningGame.appId}"`
    };
  }

  if (state.steam.running) {
    const steam = catalogApps.find((item) => item.id === "steam");
    return {
      name: "Steam",
      iconUrl: steam?.iconUrl,
      fallback: "ST",
      attribute: "data-launch-steam"
    };
  }

  return null;
}

function activityIcon(iconUrl: string | undefined, fallback: string) {
  return `
    <div class="activity-icon">
      ${iconUrl
        ? `<img src="${escapeAttribute(iconUrl)}" alt="" loading="lazy" onerror="this.hidden=true;this.nextElementSibling.hidden=false" />`
        : `<img hidden alt="" />`}
      <span ${iconUrl ? "hidden" : ""}>${escapeHtml(fallback)}</span>
    </div>
  `;
}

function rememberApp(app: RecentApp) {
  state.lastUsedApp = app;
  window.localStorage.setItem("kirito:last-used-app", JSON.stringify(app));
}

function clearLastUsedApp() {
  state.lastUsedApp = null;
  window.localStorage.removeItem("kirito:last-used-app");
}

function reconcileLastUsedApp() {
  const recent = state.lastUsedApp;
  if (!recent) return;

  if (recent.action === "launch-steam" || recent.action === "run-steam-game") {
    if (!state.steam.installed) clearLastUsedApp();
    return;
  }

  if (recent.action === "run-app") {
    const bottle = state.bottles.find((item) => item.id === recent.bottleId);
    const app = bottle?.apps.find((item) => item.id === recent.appId);
    if (!bottle || !app) clearLastUsedApp();
  }
}

function loadLastUsedApp(): RecentApp | null {
  try {
    const value = window.localStorage.getItem("kirito:last-used-app");
    return value ? JSON.parse(value) as RecentApp : null;
  } catch {
    return null;
  }
}

function loadTheme(): ThemeMode {
  const value = window.localStorage.getItem("kirito:theme");
  return value === "light" || value === "dark" || value === "auto" ? value : "auto";
}

function saveTheme(theme: ThemeMode) {
  state.theme = theme;
  window.localStorage.setItem("kirito:theme", theme);
  applyTheme();
}

function resolvedTheme() {
  if (state.theme !== "auto") return state.theme;
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function applyTheme() {
  document.documentElement.dataset.theme = resolvedTheme();
  document.documentElement.dataset.themeMode = state.theme;
}

async function searchSteam(query: string) {
  state.steamQuery = query;

  if (query.trim().length < 2) {
    state.steamResults = [];
    state.searching = false;
    render();
    return;
  }

  state.searching = true;
  render();

  try {
    state.steamResults = await apiInvoke<SteamGame[]>("search_steam", { query });
    state.status = `Found ${state.steamResults.length} Steam result${state.steamResults.length === 1 ? "" : "s"}`;
    void loadProtonRatings(state.steamResults.map((game) => game.appId));
  } catch (error) {
    state.status = error instanceof Error ? error.message : String(error);
  } finally {
    state.searching = false;
    render();
  }
}

async function loadProtonRatings(appIds: number[]) {
  const missing = [...new Set(appIds)].filter((appId) => !(appId in state.protonRatings));
  if (missing.length === 0) return;

  const results = await Promise.all(
    missing.map(async (appId) => {
      try {
        const rating = await apiInvoke<ProtonRating | null>("proton_rating", { appId });
        return [appId, rating] as const;
      } catch {
        return [appId, null] as const;
      }
    })
  );

  for (const [appId, rating] of results) {
    state.protonRatings[appId] = rating;
  }
  render();
}

function render() {
  applyTheme();
  const bottle = selectedBottle();
  const eyebrow = eyebrowText();
  if (!app) return;
  app.innerHTML = `
    <main class="shell">
      <aside class="sidebar">
        <section class="brand">
          <img class="brand-mark" src="${escapeAttribute(kiritoIconUrl)}" alt="" />
          <div>
            <h1>kirito</h1>
            <p>Run Windows apps</p>
          </div>
        </section>

        <button class="nav-item ${state.view === "install" ? "active" : ""}" data-select-install>
          <span>Install</span>
          <small>Find apps</small>
        </button>

        <button class="nav-item ${state.view === "library" ? "active" : ""}" data-select-library>
          <span>Library</span>
          <small>${libraryItemCount()}</small>
        </button>

        <button class="nav-item ${state.view === "settings" ? "active" : ""}" data-select-settings>
          <span>Settings</span>
          <small>${state.engines.length ? "Engine found" : engineEnsureInFlight || state.busy ? "Installing…" : "No engine"}</small>
        </button>

        ${sidebarActivity()}

        <div class="sidebar-bottom">
          ${sidebarInstallIndicator()}
          <div class="sidebar-actions">
            <button class="primary" data-install-any-app>Install App</button>
            <button data-refresh>Refresh</button>
          </div>
        </div>
      </aside>

      <section class="workspace">
        <header class="topbar">
          <div>
            ${eyebrow ? `<p class="eyebrow">${eyebrow}</p>` : ""}
            <h2>${pageTitle(bottle)}</h2>
          </div>
          ${topbarStatus()}
        </header>

        ${contentForView(bottle)}
      </section>
    </main>
  `;

  bindEvents();
}

function pageTitle(bottle: Bottle | null) {
  if (state.view === "install") return "Install";
  if (state.view === "library") return "Library";
  return bottle ? bottle.name : "Settings";
}

function eyebrowText() {
  if (state.view === "install") return "Find an app";
  if (state.view === "library") return "";
  return "Setup";
}

function contentForView(bottle: Bottle | null) {
  if (state.view === "install") return installHome();
  if (state.view === "library") return libraryHome();
  return settingsHome();
}

function filteredCatalogApps() {
  const query = state.installQuery.trim().toLowerCase();
  if (!query) return catalogApps;
  return catalogApps.filter((item) =>
    [
      item.name,
      item.vendor,
      item.demand,
      item.profileSignal,
      reportText(item),
      ...item.recommendedSettings,
      ...item.knownIssues,
      ...item.wantedFeatures
    ].some((value) => value.toLowerCase().includes(query))
  );
}

function installHome() {
  if (state.detailCatalogId) {
    const item = catalogApps.find((catalogItem) => catalogItem.id === state.detailCatalogId);
    if (item) return catalogDetailPage(item);
    state.detailCatalogId = null;
  }

  const results = filteredCatalogApps();
  return `
    <section class="install-panel">
      <div class="install-header">
        <div>
          <h3>Choose an app</h3>
          <p>Search, pick one below, or use your own installer.</p>
        </div>
        <button data-install-any-app>Unlisted app</button>
      </div>

      <label class="catalog-search">
        <span>Search</span>
        <input id="install-search" value="${escapeAttribute(state.installQuery)}" placeholder="Search for Office, Steam, Quicken, games, tools..." autocomplete="off" />
      </label>

      <div class="catalog-title">
        <h4>${state.installQuery.trim() ? "Results" : "Common on Mac"}</h4>
        ${state.installQuery.trim() ? `<span>${results.length} matches</span>` : ""}
      </div>

      <div class="catalog-grid">
        ${results.length ? results.map(catalogRow).join("") : empty("No matching apps", "Try another name or install an unlisted application.")}
      </div>
    </section>
  `;
}

function catalogRow(item: CatalogApp) {
  return `
    <article class="catalog-row" data-open-catalog-detail="${escapeAttribute(item.id)}">
      ${appIcon(item, "catalog-icon")}
      <div class="catalog-info">
        <strong>${escapeHtml(item.name)}</strong>
        <span>${escapeHtml(item.vendor)}</span>
      </div>
      ${reportBlock(item)}
      <button data-install-catalog="${escapeAttribute(item.id)}" ${catalogActionDisabled(item)}>${catalogActionLabel(item)}</button>
    </article>
  `;
}

function catalogDetailPage(item: CatalogApp) {
  const rating = item.steamAppId ? state.protonRatings[item.steamAppId] : null;
  const ratingLabel = item.reportSource === "protondb"
    ? rating
      ? protonTierLabel(rating.tier)
      : "Checking"
    : item.reportSource === "community" ? "Experimental community runtime" : "WineHQ AppDB";
  const ratingDetail = item.reportSource === "protondb"
    ? rating
      ? `${rating.total ?? "Community"} reports on ProtonDB`
      : "Loading external report data"
    : "External app reports, not a kirito guarantee";
  const trust = item.id === "minecraft-bedrock" ? "Microsoft purchase required" : item.id === "steam"
    ? "Trusted automatic install"
    : item.steamAppId
      ? "Install through Steam"
      : "Manual installer for now";
  const trustDetail = item.id === "minecraft-bedrock" ? "Uses a checksum-pinned third-party beta runtime and Xodus to sign in and download your PC purchase. Some runtime modifications are not yet published as source." : item.id === "steam"
    ? "kirito downloads the Steam setup from Valve and installs it into its own bottle."
    : item.steamAppId
      ? "kirito opens Steam for the install instead of downloading game files directly."
      : "kirito opens the installer picker until verified download manifests are wired in.";

  return `
    <section class="catalog-detail-page">
      <button class="back-button" data-back-catalog>Back</button>

      <div class="detail-hero">
        ${appIcon(item, "detail-icon-large")}
        <div>
          <h4>${escapeHtml(item.name)}</h4>
          <strong>${escapeHtml(item.vendor)}</strong>
          <p>${escapeHtml(item.summary)}</p>
        </div>
        <button data-install-catalog="${escapeAttribute(item.id)}" ${catalogActionDisabled(item)}>${catalogActionLabel(item)}</button>
      </div>

      <div class="rating-grid">
        ${detailMetric("External reports", ratingLabel, ratingDetail)}
        ${detailMetric("Install trust", trust, trustDetail)}
        ${detailMetric("Profile signal", item.demand, item.profileSignal)}
      </div>

      <div class="profile-grid">
        ${item.installPlan ? installPlanCard(item.installPlan) : ""}
        ${profileList("Recommended settings", item.recommendedSettings)}
        ${profileList("Watch for", item.knownIssues)}
        ${profileList("Needed next", item.wantedFeatures)}
      </div>

      <div class="source-grid detail-sources">
        ${sourcePill("App info", item.infoSource)}
        ${sourcePill("Installer", item.installSource)}
        ${sourcePill("Reports", item.reportSource === "protondb" ? "ProtonDB" : item.reportSource === "community" ? "Community beta" : "WineHQ AppDB")}
        ${sourcePill("Action", item.steamAppId ? "Open in Steam" : item.id === "steam" ? "Automatic setup" : "Choose installer")}
      </div>
    </section>
  `;
}

function installPlanCard(steps: InstallPlanStep[]) {
  return `
    <article class="profile-card install-recipe-card">
      <h5>Install recipe</h5>
      <ol class="install-recipe">
        ${steps.map((step) => `
          <li data-kind="${escapeAttribute(step.kind)}">
            <strong>${escapeHtml(step.title)}</strong>
            <span>${escapeHtml(step.detail)}</span>
          </li>
        `).join("")}
      </ol>
    </article>
  `;
}

function profileList(title: string, items: string[]) {
  return `
    <article class="profile-card">
      <h5>${escapeHtml(title)}</h5>
      <ul>
        ${items.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}
      </ul>
    </article>
  `;
}

function detailMetric(label: string, title: string, detail: string) {
  return `
    <article class="detail-metric">
      <span>${escapeHtml(label)}</span>
      <strong>${escapeHtml(title)}</strong>
      <p>${escapeHtml(detail)}</p>
    </article>
  `;
}

function sourcePill(label: string, value: string) {
  return `
    <div class="source-pill">
      <span>${escapeHtml(label)}</span>
      <strong>${escapeHtml(value)}</strong>
    </div>
  `;
}

function reportBlock(item: CatalogApp) {
  if (item.reportSource === "community") return `<div class="compat compat-source"><strong>Experimental</strong><span>Community runtime</span></div>`;
  if (item.reportSource === "winehq") {
    return `
      <div class="compat compat-source">
        <strong>Reports</strong>
        <span>WineHQ AppDB</span>
      </div>
    `;
  }

  const text = reportText(item);
  const source = "ProtonDB";
  const tier = item.steamAppId ? state.protonRatings[item.steamAppId]?.tier : null;
  return `
    <div class="compat ${tier ? `tier-${escapeAttribute(tier)}` : ""}">
      <strong>${escapeHtml(text)}</strong>
      <span>${source}</span>
    </div>
  `;
}

function reportText(item: CatalogApp) {
  if (item.reportSource === "community") return "Experimental";
  if (item.reportSource === "protondb" && item.steamAppId) {
    const rating = state.protonRatings[item.steamAppId];
    if (rating) return protonTierLabel(rating.tier);
    return "Checking...";
  }
  return "WineHQ AppDB";
}

function protonTierLabel(tier: string) {
  const labels: Record<string, string> = {
    platinum: "Platinum",
    gold: "Gold",
    silver: "Silver",
    bronze: "Bronze",
    borked: "Borked"
  };
  return labels[tier.toLowerCase()] ?? "Unknown";
}

function catalogActionLabel(item: CatalogApp) {
  if (item.id === "minecraft-bedrock") return state.bedrock.installed ? "Play" : "Set up Bedrock";
  if (item.id === "steam" && state.steam.running) return "Running";
  if (item.id === "steam" && state.steam.installed) return "Open";
  return "Install";
}

function catalogActionDisabled(item: CatalogApp) {
  if (state.busy) return "disabled";
  return item.id === "steam" && state.steam.running ? "disabled" : "";
}

function libraryItemCount() {
  return launcherItems().length + state.steam.games.length;
}

function appIsRunning(appId: string) {
  return state.runtime.appIds.includes(appId);
}

function steamGameIsRunning(appId: number) {
  return state.runtime.steamAppIds.includes(appId);
}

function steamGameIsStarting(appId: number) {
  return launchingSteamAppIds.has(appId);
}

function launcherItems() {
  return state.bottles.flatMap((bottle) =>
    bottle.apps.map((item) => ({ ...item, bottleId: bottle.id, bottleName: bottle.name }))
  );
}

function libraryHome() {
  if (state.appSettingsTarget) {
    return appSettingsPage(state.appSettingsTarget);
  }

  return `
    <section class="library-panel">
      <div class="search-heading">
        <div>
          <h3>Installed apps</h3>
          <p>Open anything you have added to kirito.</p>
        </div>
        <div class="button-row">
          <button data-refresh>Refresh</button>
          <button class="primary" data-select-install>Add App</button>
        </div>
      </div>
      <div class="installed-list">
        ${allInstalledItems()}
      </div>
    </section>
  `;
}

function quickCard(title: string, detail: string, action: string, attribute: string) {
  return `
    <article class="quick-card">
      <div>
        <strong>${title}</strong>
        <p>${detail}</p>
      </div>
      <button ${attribute}>${action}</button>
    </article>
  `;
}

function allInstalledItems() {
  const apps = launcherItems();
  const steamGames = state.steam.games;

  if (apps.length === 0 && steamGames.length === 0) {
    return empty("No apps installed yet", "Install a Windows app or set up Steam to start building your library.");
  }

  return [
    ...apps.map((item) => genericAppRow(item.name, item.bottleName, item.id, item.bottleId)),
    ...steamGames.map(installedGameCard)
  ].join("");
}

function genericAppRow(name: string, source: string, appId: string, bottleId: string) {
  const catalogItem = catalogAppForName(name);
  const isSteam = normalizeAppName(name) === "steam";
  const isRunning = isSteam ? state.steam.running : appIsRunning(appId);
  const settingsKey = isSteam ? "steam-client" : `app:${appId}`;
  const actionLabel = isRunning ? "Running" : "Open";
  const actionAttribute = isSteam
    ? `data-launch-steam ${isRunning ? "disabled" : ""}`
    : `data-run-app="${appId}" data-bottle="${bottleId}" ${isRunning ? "disabled" : ""}`;
  return `
    <article class="installed-game app-row-card">
      ${catalogItem ? appIcon(catalogItem, "app-icon") : kiritoFallbackIcon("app-icon")}
      <div class="installed-body">
        <strong>${escapeHtml(name)}</strong>
        <span>${escapeHtml(source)}</span>
      </div>
      <div class="installed-actions">
        <button class="icon-button" title="App settings" aria-label="App settings" data-app-settings="${escapeAttribute(settingsKey)}" data-bottle="${escapeAttribute(bottleId)}" data-app-name="${escapeAttribute(name)}" data-app-subtitle="${escapeAttribute(source)}">⚙</button>
        <button class="primary" ${actionAttribute}>${actionLabel}</button>
      </div>
    </article>
  `;
}

function appSettingsPage(target: AppSettingsTarget) {
  const bottle = state.bottles.find((item) => item.id === target.bottleId);
  if (!bottle) return empty("App settings unavailable", "Refresh kirito and try again.");

  const settings = bottle.appSettings[target.appKey] ?? emptyAppSettings();
  const recommendation = appSettingsRecommendation(target);
  return `
    <section class="panel app-settings-panel">
      <div class="panel-title">
        <div>
          <h3>${escapeHtml(target.name)}</h3>
          <p>${escapeHtml(target.subtitle)}</p>
        </div>
        <button data-back-library>Back</button>
      </div>

      <div class="settings-grid app-override-grid">
        ${engineOverrideControl(settings.engineExecutable)}
        ${overrideSelect("app-vsync", "VSync", settings.vsync, `Bottle default: ${bottle.settings.vsync ? "On" : "Off"}`)}
        ${overrideSelect("app-d3dmetal", "D3DMetal", settings.d3dMetal, `Bottle default: ${bottle.settings.d3dMetal ? "On" : "Off"}`)}
        ${overrideSelect("app-dxvk", "DXVK", settings.dxvk, `Bottle default: ${bottle.settings.dxvk ? "On" : "Off"}`)}
        ${overrideSelect("app-msync", "MSync", settings.msync, `Bottle default: ${bottle.settings.msync ? "On" : "Off"}`)}
        ${overrideSelect("app-retina", "Retina", settings.retina, `Bottle default: ${bottle.settings.retina ? "On" : "Off"}`)}
      </div>

      ${compatibilityDetailPanel(target, settings, recommendation)}

      ${recommendation ? recommendationBlock(recommendation, settings) : ""}

      <div class="preset-row">
        <button data-app-preset="metal">Prefer Metal</button>
        <button data-app-preset="flicker">Reduce flicker</button>
      </div>

      <div class="button-row">
        <button data-reset-app-settings>Use Bottle Defaults</button>
        <button class="primary" data-save-app-settings>Save Settings</button>
      </div>
    </section>
  `;
}

function compatibilityDetailPanel(
  target: AppSettingsTarget,
  current: AppSettings,
  recommendation: AppSettingsRecommendation | null
) {
  if (!target.appKey.startsWith("steam:")) return "";
  const appId = Number(target.appKey.replace("steam:", ""));
  if (!Number.isFinite(appId)) return "";

  const rating = state.protonRatings[appId] ?? null;
  const antiCheat = antiCheatReportForName(target.name);
  const launcherRisk = steamGameLauncherRisk(target.name);
  const controllerDetail = controllerCompatibilityDetail();
  const risks = [
    antiCheat && ["Broken", "Denied"].includes(antiCheat.status) ? `Anti-cheat ${antiCheat.status}` : null,
    launcherRisk ? "Launcher/login handoff risk" : null,
    controllerDetail.level === "warn" ? controllerDetail.text : null
  ].filter((item): item is string => Boolean(item));

  return `
    <section class="compatibility-detail">
      <div class="compatibility-header">
        <div>
          <span class="compatibility-kicker">Compatibility detail</span>
          <strong>${escapeHtml(target.name)}</strong>
        </div>
        <div class="compatibility-header-actions">
          <span class="pill">${escapeHtml(risks.length ? `${risks.length} risk${risks.length === 1 ? "" : "s"}` : "No high-risk signal")}</span>
          <button
            data-open-feedback="compatibility"
            data-feedback-app-name="${escapeAttribute(target.name)}"
            data-feedback-source="Steam"
            data-feedback-steam-app-id="${appId}"
            data-feedback-settings="${escapeAttribute(settingsSummary(current))}"
            data-feedback-notes="${escapeAttribute(compatibilityReportNotes(rating, antiCheat, controllerDetail, risks, recommendation))}"
          >Report result</button>
        </div>
      </div>
      <div class="compatibility-grid">
        ${compatibilityMetric("ProtonDB", rating ? protonTierLabel(rating.tier) : "No rating loaded", rating ? `${rating.total ?? "Community"} reports` : "External report unavailable")}
        ${compatibilityMetric("Anti-cheat", antiCheat ? antiCheat.status : "No match", antiCheatDetail(antiCheat))}
        ${compatibilityMetric("Controller", controllerDetail.title, controllerDetail.text)}
        ${compatibilityMetric("Current profile", "Overrides", settingsSummary(current))}
        ${compatibilityMetric("Recommended", recommendation ? recommendation.confidence : "None", recommendation ? settingsSummary(recommendation.settings) : "No per-game recommendation")}
        ${compatibilityMetric("Known risks", risks.length ? "Review" : "Low", risks.length ? risks.join("; ") : "No anti-cheat, launcher, or controller warning from current signals")}
      </div>
    </section>
  `;
}

function compatibilityReportNotes(
  rating: ProtonRating | null,
  antiCheat: AntiCheatReport | null,
  controllerDetail: ReturnType<typeof controllerCompatibilityDetail>,
  risks: string[],
  recommendation: AppSettingsRecommendation | null
) {
  return [
    rating ? `ProtonDB: ${protonTierLabel(rating.tier)}${rating.total ? ` (${rating.total} reports)` : ""}` : "ProtonDB: no rating loaded",
    antiCheat ? `Anti-cheat: ${antiCheat.status}; ${antiCheatDetail(antiCheat)}` : "Anti-cheat: no match",
    `Controller: ${controllerDetail.title}; ${controllerDetail.text}`,
    risks.length ? `Known risks: ${risks.join("; ")}` : "Known risks: none from current signals",
    recommendation ? `Kirito recommendation: ${recommendation.confidence}; ${settingsSummary(recommendation.settings)}` : "Kirito recommendation: none"
  ].join("\n");
}

function antiCheatDetail(report: AntiCheatReport | null) {
  if (!report) return "No external anti-cheat match";
  const engines = report.antiCheats.length ? report.antiCheats.join(" + ") : "anti-cheat engine not listed";
  return `${engines}${report.updated ? `, updated ${report.updated}` : ""}`;
}

function controllerCompatibilityDetail() {
  const diagnostics = state.controllerDiagnostics;
  if (!diagnostics) {
    return { title: "Checking", text: "Controller check pending", level: "info" as const };
  }
  if (diagnostics.devices.length === 0) {
    return { title: "None detected", text: "No controller detected by macOS", level: "warn" as const };
  }
  return {
    title: `${diagnostics.devices.length} detected`,
    text: diagnostics.devices.slice(0, 2).map((device) => `${device.name} (${device.transport})`).join(", "),
    level: "info" as const
  };
}

function compatibilityMetric(label: string, value: string, detail: string) {
  return `
    <article class="compatibility-metric">
      <span>${escapeHtml(label)}</span>
      <strong>${escapeHtml(value)}</strong>
      <small>${escapeHtml(detail)}</small>
    </article>
  `;
}

function recommendationBlock(recommendation: AppSettingsRecommendation, current: AppSettings) {
  const currentSummary = settingsSummary(current);
  const recommendedSummary = settingsSummary(recommendation.settings);
  return `
    <section class="recommendation-panel">
      <div class="recommendation-copy">
        <span>${escapeHtml(recommendation.confidence)} confidence</span>
        <strong>${escapeHtml(recommendation.title)}</strong>
        <p>${escapeHtml(recommendation.rationale.join(" "))}</p>
        ${recommendation.warnings.length ? `<p class="recommendation-warning">${escapeHtml(recommendation.warnings.join(" "))}</p>` : ""}
      </div>
      <div class="recommendation-compare">
        <small>Current</small>
        <code>${escapeHtml(currentSummary)}</code>
        <small>Recommended</small>
        <code>${escapeHtml(recommendedSummary)}</code>
      </div>
      <button class="primary" data-apply-recommended-settings ${sameSettings(current, recommendation.settings) ? "disabled" : ""}>Apply recommended</button>
    </section>
  `;
}

function emptyAppSettings(): AppSettings {
  return {
    engineExecutable: null,
    d3dMetal: null,
    dxvk: null,
    msync: null,
    retina: null,
    vsync: null
  };
}

function appSettingsRecommendation(target: AppSettingsTarget): AppSettingsRecommendation | null {
  if (!target.appKey.startsWith("steam:")) return null;
  const appId = Number(target.appKey.replace("steam:", ""));
  if (!Number.isFinite(appId)) return null;

  const game = state.steam.games.find((item) => item.appId === appId)
    ?? state.steamResults.find((item) => item.appId === appId);
  const name = game?.name ?? target.name;
  const rating = state.protonRatings[appId] ?? null;
  const antiCheat = antiCheatReportForName(name);
  const launcherRisk = steamGameLauncherRisk(name);
  const localRule = compatibilityRuleFor(appId, name);
  const settings = emptyAppSettings();
  const rationale: string[] = [];
  const warnings: string[] = [];
  let confidence: AppSettingsRecommendation["confidence"] = "Low";
  let title = "Use conservative game defaults";

  settings.vsync = true;
  settings.retina = false;
  rationale.push("Keep VSync on and Retina off because those are low-risk stability defaults for Wine launchers and games on macOS.");

  if (localRule) {
    title = localRule.title ?? title;
    confidence = localRule.confidence ?? confidence;
    applyCompatibilityRuleSettings(settings, localRule.settings);
    rationale.push(...(localRule.rationale ?? []));
    warnings.push(...(localRule.warnings ?? []));
  } else if (launcherRisk) {
    title = "Launcher-safe Steam profile";
    confidence = "Medium";
    settings.retina = false;
    settings.vsync = true;
    rationale.push("This title is launcher-heavy, so the recommendation avoids high-DPI launcher rendering and keeps presentation synchronized.");
  } else if (rating && ["platinum", "gold"].includes(rating.tier.toLowerCase())) {
    title = "Modern Steam game profile";
    confidence = "Medium";
    settings.d3dMetal = true;
    settings.dxvk = false;
    settings.msync = true;
    rationale.push(`ProtonDB reports ${protonTierLabel(rating.tier)}, so Kirito can recommend the modern D3DMetal/MSync path as a starting point.`);
  } else if (rating && ["borked", "unknown"].includes(rating.tier.toLowerCase())) {
    title = "Compatibility unknown";
    confidence = "Low";
    rationale.push(`ProtonDB reports ${protonTierLabel(rating.tier)}, so Kirito avoids guessing a graphics backend.`);
  } else {
    rationale.push("No strong compatibility signal is available, so Kirito avoids changing D3DMetal, DXVK, or MSync.");
  }

  if (antiCheat && ["Broken", "Denied"].includes(antiCheat.status)) {
    confidence = "Low";
    warnings.push(`Anti-cheat is reported as ${antiCheat.status}; settings cannot usually bypass that.`);
  }

  return { settings, confidence, title, rationale, warnings };
}

function compatibilityRuleFor(appId: number, name: string) {
  const normalized = normalizeAppName(name);
  return localCompatibilityRules.appRules.find((rule) => {
    if (rule.appId === appId) return true;
    if (!rule.name) return false;
    return normalizeAppName(rule.name) === normalized;
  }) ?? null;
}

function applyCompatibilityRuleSettings(settings: AppSettings, ruleSettings?: CompatibilityRuleSettings) {
  if (!ruleSettings) return;
  if (typeof ruleSettings.d3dMetal === "boolean") settings.d3dMetal = ruleSettings.d3dMetal;
  if (typeof ruleSettings.dxvk === "boolean") settings.dxvk = ruleSettings.dxvk;
  if (typeof ruleSettings.msync === "boolean") settings.msync = ruleSettings.msync;
  if (typeof ruleSettings.retina === "boolean") settings.retina = ruleSettings.retina;
  if (typeof ruleSettings.vsync === "boolean") settings.vsync = ruleSettings.vsync;
}

function settingsSummary(settings: AppSettings) {
  return [
    settingToken("Metal", settings.d3dMetal),
    settingToken("DXVK", settings.dxvk),
    settingToken("MSync", settings.msync),
    settingToken("Retina", settings.retina),
    settingToken("VSync", settings.vsync)
  ].join(" ");
}

function settingToken(label: string, value: boolean | null) {
  if (value === null) return `${label}: default`;
  return `${label}: ${value ? "on" : "off"}`;
}

function sameSettings(left: AppSettings, right: AppSettings) {
  return left.engineExecutable === right.engineExecutable
    && left.d3dMetal === right.d3dMetal
    && left.dxvk === right.dxvk
    && left.msync === right.msync
    && left.retina === right.retina
    && left.vsync === right.vsync;
}

function engineOverrideControl(value: string | null) {
  const options = [
    `<option value="" ${value ? "" : "selected"}>Use bottle default</option>`,
    ...state.engines.map((engine) =>
      `<option value="${escapeAttribute(engine.executable)}" ${value === engine.executable ? "selected" : ""}>${escapeHtml(engine.displayName)}</option>`
    )
  ].join("");

  return `
    <label class="setting-select">
      <span>Wine engine</span>
      <select id="app-engine">${options}</select>
    </label>
  `;
}

function overrideSelect(id: string, label: string, value: boolean | null, fallback: string) {
  return `
    <label class="setting-select">
      <span>${escapeHtml(label)}</span>
      <select id="${id}">
        <option value="" ${value === null ? "selected" : ""}>${escapeHtml(fallback)}</option>
        <option value="true" ${value === true ? "selected" : ""}>On</option>
        <option value="false" ${value === false ? "selected" : ""}>Off</option>
      </select>
    </label>
  `;
}

function readOverride(id: string): boolean | null {
  const value = document.querySelector<HTMLSelectElement>(`#${id}`)?.value ?? "";
  if (value === "true") return true;
  if (value === "false") return false;
  return null;
}

function favicon(domain: string) {
  return `https://www.google.com/s2/favicons?domain=${encodeURIComponent(domain)}&sz=128`;
}

function catalogAppForName(name: string) {
  const normalizedName = normalizeAppName(name);
  return catalogApps.find((item) => {
    const normalizedCatalogName = normalizeAppName(item.name);
    return normalizedName === normalizedCatalogName || normalizedName.includes(normalizedCatalogName);
  });
}

function appIcon(item: CatalogApp, className: string) {
  return `
    <div class="${className}">
      <img src="${escapeAttribute(item.iconUrl)}" alt="" loading="lazy" onerror="this.hidden=true;this.nextElementSibling.hidden=false" />
      <span hidden>${escapeHtml(item.badge)}</span>
    </div>
  `;
}

function kiritoFallbackIcon(className: string) {
  return `
    <div class="${className} fallback">
      <img src="${escapeAttribute(kiritoIconUrl)}" alt="" loading="lazy" />
      <span hidden>K</span>
    </div>
  `;
}

function normalizeAppName(name: string) {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

function bottleButton(bottle: Bottle) {
  const active = state.view === "settings" && bottle.id === state.selectedBottleId ? "active" : "";
  return `
    <button class="nav-item ${active}" data-select-bottle="${bottle.id}">
      <span>${escapeHtml(bottle.name)}</span>
      <small>${escapeHtml(bottle.windowsVersion)}</small>
    </button>
  `;
}

function steamHome() {
  return `
    <section class="steam-hero">
      <div class="steam-copy">
        <span class="badge">${steamBadgeText()}</span>
        <h3>Steam games can live beside your other apps.</h3>
        <p>Install Steam once, sign in normally, and kirito will find installed games.</p>
      </div>
      <div class="steam-actions">
        <button class="primary large" data-install-steam ${state.steam.running ? "disabled" : ""}>${steamPrimaryActionText()}</button>
        <button class="large" data-launch-steam ${state.steam.installed && !state.steam.running ? "" : "disabled"}>${state.steam.running ? "Open" : "Launch Steam"}</button>
      </div>
    </section>

    <section class="setup-strip">
      ${step("1", "Prepare bottle", state.steam.installed ? "Done" : "Windows 10 + runtimes")}
      ${step("2", "Install Steam", state.steam.installed ? "Ready" : "Valve installer")}
      ${step("3", "Play", "Search or use your library")}
    </section>

    ${steamDiagnosticsPanel()}

    <section class="library-panel">
      <div class="search-heading">
        <div>
          <h3>Installed through Steam</h3>
          <p>Refresh after adding games in Steam.</p>
        </div>
        <button data-refresh-steam-library ${state.steam.installed ? "" : "disabled"}>Refresh Library</button>
      </div>
      <div class="installed-list">
        ${installedSteamGames()}
      </div>
    </section>

    <section class="search-panel">
      <div class="search-heading">
        <div>
          <h3>Store search</h3>
          <p>Find a game and open its Steam page.</p>
        </div>
        <span>${state.searching ? "Searching..." : "Store search"}</span>
      </div>
      <label class="search-box">
        <span>Search</span>
        <input id="steam-search" value="${escapeAttribute(state.steamQuery)}" placeholder="Try: Hades, Stardew Valley, Portal" autocomplete="off" />
      </label>
      <div class="game-grid">
        ${steamResults()}
      </div>
    </section>
  `;
}

function steamBadgeText() {
  if (state.steam.running) return "Steam is open";
  if (state.steam.installed) return "Steam is installed";
  return "Optional setup";
}

function steamPrimaryActionText() {
  if (state.steam.running) return "Running";
  if (state.steam.installed) return "Open Steam";
  return "Install Steam";
}

function steamSettingsValue() {
  if (state.steam.running) return "Open";
  if (state.steam.installed) return "Installed";
  return "Not set up";
}

function installedSteamGames() {
  if (!state.steam.installed) {
    return empty("Install Steam first", "Once Steam is installed, games you install there will show up here.");
  }

  if (state.steam.games.length === 0) {
    return empty("No Steam games found yet", "Install a game in Steam, then press Refresh Library.");
  }

  return state.steam.games.map(installedGameCard).join("");
}

function steamDiagnosticsPanel() {
  if (!state.steam.installed) return "";
  const diagnostics = steamDiagnostics();
  if (diagnostics.length === 0 && !state.launchFailure) return "";

  return `
    <section class="panel diagnostics-panel">
      <div class="panel-title">
        <h3>Steam diagnostics</h3>
        <span class="pill">${diagnostics.length} ${diagnostics.length === 1 ? "item" : "items"}</span>
      </div>
      ${state.launchFailure ? launchFailurePanel(state.launchFailure) : ""}
      <div class="diagnostic-list">
        ${diagnostics.map((item) => `
          <article class="diagnostic-item ${item.level}">
            <strong>${escapeHtml(item.title)}</strong>
            <small>${escapeHtml(item.detail)}</small>
          </article>
        `).join("")}
      </div>
    </section>
  `;
}

function launchFailurePanel(failure: LaunchFailure) {
  return `
    <article class="launch-failure">
      <div>
        <strong>${escapeHtml(failure.title)}</strong>
        <small>${escapeHtml(failure.detail)}</small>
      </div>
      <div class="launch-failure-actions">
        <button
          data-open-feedback="compatibility"
          data-feedback-app-name="${escapeAttribute(failure.gameName)}"
          data-feedback-source="Steam"
          data-feedback-steam-app-id="${failure.appId}"
          data-feedback-settings="${escapeAttribute(steamGameReportSettings(failure.appId))}"
          data-feedback-notes="${escapeAttribute(launchFailureReportNotes(failure))}"
        >Report</button>
        <button data-launch-failure-action="${escapeAttribute(failure.action)}" data-launch-failure-app="${failure.appId}">${escapeHtml(failure.actionLabel)}</button>
      </div>
    </article>
  `;
}

function steamGameReportSettings(appId: number) {
  const bottle = state.steam.bottleId
    ? state.bottles.find((item) => item.id === state.steam.bottleId)
    : null;
  const overrides = bottle?.appSettings[`steam:${appId}`] ?? emptyAppSettings();
  if (!bottle) return settingsSummary(overrides);
  return [
    `App overrides: ${settingsSummary(overrides)}`,
    `Bottle defaults: Metal ${onOff(bottle.settings.d3dMetal)}, DXVK ${onOff(bottle.settings.dxvk)}, MSync ${onOff(bottle.settings.msync)}, Retina ${onOff(bottle.settings.retina)}, VSync ${onOff(bottle.settings.vsync)}`
  ].join("\n");
}

function launchFailureReportNotes(failure: LaunchFailure) {
  const rating = state.protonRatings[failure.appId] ?? null;
  const antiCheat = antiCheatReportForName(failure.gameName);
  const controllerDetail = controllerCompatibilityDetail();
  const launcherRisk = steamGameLauncherRisk(failure.gameName);
  return [
    `Launch failure: ${failure.title}`,
    failure.detail,
    `Suggested action: ${failure.actionLabel}`,
    rating ? `ProtonDB: ${protonTierLabel(rating.tier)}${rating.total ? ` (${rating.total} reports)` : ""}` : "ProtonDB: no rating loaded",
    antiCheat ? `Anti-cheat: ${antiCheat.status}; ${antiCheatDetail(antiCheat)}` : "Anti-cheat: no match",
    `Controller: ${controllerDetail.title}; ${controllerDetail.text}`,
    launcherRisk ? "Launcher risk: yes" : "Launcher risk: no local signal"
  ].join("\n");
}

function onOff(value: boolean) {
  return value ? "on" : "off";
}

function explainSteamLaunchFailure(appId: number): LaunchFailure {
  const game = state.steam.games.find((item) => item.appId === appId)
    ?? state.steamResults.find((item) => item.appId === appId);
  const gameName = game?.name ?? "Steam game";
  const antiCheat = antiCheatReportForName(gameName);
  if (appId === 730 && !state.engines.some((engine) => engine.d3dMetalCapable)) {
    return {
      appId,
      gameName,
      title: "CS2 needs a D3DMetal engine",
      detail: "CrossOver parity requires CrossOver or Game Porting Toolkit with D3DMetal. Stock Wine Stable cannot run CS2 well on Mac.",
      action: "settings",
      actionLabel: "Engine settings"
    };
  }
  if (antiCheat && ["Broken", "Denied"].includes(antiCheat.status)) {
    const engines = antiCheat.antiCheats.length ? ` using ${antiCheat.antiCheats.join(" + ")}` : "";
    return {
      appId,
      gameName,
      title: `${gameName} is likely blocked by anti-cheat`,
      detail: `AreWeAntiCheatYet reports ${antiCheat.status}${engines}. Wine/CrossOver usually cannot fix this locally.`,
      action: "settings",
      actionLabel: "Game settings"
    };
  }

  if (!state.steam.running) {
    return {
      appId,
      gameName,
      title: `${gameName} did not start because Steam is not running`,
      detail: "Steam may have exited during handoff. Start Steam again, then retry the game.",
      action: "retry",
      actionLabel: "Retry"
    };
  }

  if (steamGameLauncherRisk(gameName)) {
    return {
      appId,
      gameName,
      title: `${gameName} may be stuck behind a launcher or web login`,
      detail: "Launcher-heavy games commonly fail during account, update, or embedded browser handoff. Clear Steam web cache, then retry.",
      action: "clear-cache",
      actionLabel: "Clear cache"
    };
  }

  if (state.controllerDiagnostics?.devices.length === 0) {
    return {
      appId,
      gameName,
      title: `${gameName} did not appear running`,
      detail: "No controller is detected by macOS. If this game expects controller input, connect or pair one before retrying.",
      action: "retry",
      actionLabel: "Retry"
    };
  }

  return {
    appId,
    gameName,
    title: `${gameName} did not appear running`,
    detail: "Steam accepted the launch, but Kirito did not detect the game process. Try stopping Steam, then launch the game again.",
    action: "stop-steam",
    actionLabel: "Stop Steam"
  };
}

function steamDiagnostics(): SteamDiagnostic[] {
  const diagnostics: SteamDiagnostic[] = [];
  const controllerDiagnostic = controllerSteamDiagnostic();
  if (controllerDiagnostic) {
    diagnostics.push(controllerDiagnostic);
  }

  if (state.steam.running) {
    diagnostics.push({
      title: "Steam is running",
      detail: "If the store, login, or web helper hangs, use Settings > Steam troubleshooting to stop Steam or clear web cache.",
      level: "info"
    });
  }

  const flaggedGames = state.steam.games.filter((game) => steamGameCompatibilityRisk(game.name));
  if (flaggedGames.length > 0) {
    diagnostics.push({
      title: "Anti-cheat blockers detected",
      detail: `${flaggedGames.slice(0, 3).map((game) => antiCheatSummary(game)).join(", ")}${flaggedGames.length > 3 ? ` +${flaggedGames.length - 3} more` : ""}. These may fail before gameplay under Wine/CrossOver.`,
      level: "warn"
    });
  }

  const launcherGames = state.steam.games.filter((game) => steamGameLauncherRisk(game.name));
  if (launcherGames.length > 0) {
    diagnostics.push({
      title: "Launcher-heavy games detected",
      detail: "These often need per-game graphics settings or a clean launcher cache if login windows get stuck.",
      level: "info"
    });
  }

  return diagnostics;
}

function controllerSteamDiagnostic(): SteamDiagnostic | null {
  const controllerDiagnostics = state.controllerDiagnostics;
  if (!controllerDiagnostics) {
    return {
      title: "Controller check pending",
      detail: "Kirito is checking macOS USB and Bluetooth devices.",
      level: "info"
    };
  }

  if (controllerDiagnostics.devices.length === 0) {
    return {
      title: "No controller detected by macOS",
      detail: "Connect or pair a controller before launching controller-dependent Steam games.",
      level: "warn"
    };
  }

  const deviceText = controllerDiagnostics.devices
    .slice(0, 3)
    .map((device) => `${device.name} (${device.transport})`)
    .join(", ");
  const extra = controllerDiagnostics.devices.length > 3
    ? ` +${controllerDiagnostics.devices.length - 3} more`
    : "";
  return {
    title: "Controller detected",
    detail: `${deviceText}${extra}. Launch games from Steam/Kirito so Steam Input can hook correctly.`,
    level: "info"
  };
}

function steamGameCompatibilityRisk(name: string) {
  const report = antiCheatReportForName(name);
  if (report) {
    return ["Broken", "Denied"].includes(report.status);
  }

  const normalized = normalizeAppName(name);
  return localCompatibilityRules.antiCheatRiskTokens.some((token) => normalized.includes(token));
}

function steamGameLauncherRisk(name: string) {
  const normalized = normalizeAppName(name);
  return localCompatibilityRules.launcherRiskTokens.some((token) => normalized.includes(token));
}

function antiCheatReportForName(name: string) {
  return state.antiCheatReports[normalizeAppName(name)] ?? null;
}

function antiCheatSummary(game: InstalledSteamGame) {
  const report = antiCheatReportForName(game.name);
  if (!report) return game.name;
  const engines = report.antiCheats.length ? `, ${report.antiCheats.join(" + ")}` : "";
  return `${game.name} (${report.status}${engines})`;
}

function antiCheatLine(name: string) {
  const report = antiCheatReportForName(name);
  if (!report) return null;
  const engines = report.antiCheats.length ? `, ${report.antiCheats.join(" + ")}` : "";
  return `Anti-cheat ${report.status}${engines}`;
}

function installedGameCard(game: InstalledSteamGame) {
  const rating = state.protonRatings[game.appId];
  const bottleId = state.steam.bottleId ?? "";
  const isRunning = steamGameIsRunning(game.appId);
  const isStarting = steamGameIsStarting(game.appId);
  const antiCheat = antiCheatLine(game.name);
  return `
    <article class="installed-game app-row-card steam-game-row">
      ${steamGameArt(game)}
      <div class="installed-body">
        <strong>${escapeHtml(game.name)}</strong>
        <span>${escapeHtml(antiCheat ?? (rating ? `ProtonDB ${protonTierLabel(rating.tier)}` : "Checking ProtonDB..."))}</span>
      </div>
      <div class="installed-actions">
        <button class="icon-button" title="Game settings" aria-label="Game settings" data-app-settings="${escapeAttribute(`steam:${game.appId}`)}" data-bottle="${escapeAttribute(bottleId)}" data-app-name="${escapeAttribute(game.name)}" data-app-subtitle="Steam game" ${bottleId ? "" : "disabled"}>⚙</button>
        <button class="primary" data-run-steam-game="${game.appId}" ${state.steam.installed && !isRunning && !isStarting ? "" : "disabled"}>${isRunning ? "Running" : isStarting ? "Starting..." : "Play"}</button>
      </div>
    </article>
  `;
}

function steamGameArt(game: InstalledSteamGame) {
  return `
    <div class="installed-art steam-game-art">
      <img src="${escapeAttribute(game.image)}" alt="" loading="lazy" onerror="this.hidden=true;this.nextElementSibling.hidden=false" />
      <span hidden>${escapeHtml(steamGameInitials(game.name))}</span>
    </div>
  `;
}

function steamGameInitials(name: string) {
  const words = name
    .replace(/[™®©]/g, "")
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean);
  return (words.slice(0, 2).map((word) => word[0]).join("") || "ST").toUpperCase();
}

function steamResults() {
  if (state.searching && state.steamResults.length === 0) {
    return empty("Searching Steam", "Results will appear here in a moment.");
  }

  if (state.steamQuery.trim().length < 2) {
    return empty("Search for a game", "Type a game name above. kirito will look it up on Steam.");
  }

  if (state.steamResults.length === 0) {
    return empty("No games found", "Try a shorter or more common game name.");
  }

  return state.steamResults.map(gameCard).join("");
}

function gameCard(game: SteamGame) {
  const rating = state.protonRatings[game.appId];
  const isRunning = steamGameIsRunning(game.appId);
  const isStarting = steamGameIsStarting(game.appId);
  return `
    <article class="game-card">
      <div class="game-art">
        ${game.image ? `<img src="${escapeAttribute(game.image)}" alt="" />` : `<div class="game-fallback">Steam</div>`}
      </div>
      <div class="game-body">
        <strong>${escapeHtml(game.name)}</strong>
        <span>${escapeHtml(rating ? `ProtonDB ${protonTierLabel(rating.tier)}` : game.price ?? "Checking ProtonDB...")}</span>
      </div>
      <div class="game-actions">
        <button class="primary" data-open-steam-game="${game.appId}" ${state.steam.installed ? "" : "disabled"}>Open in Steam</button>
        <button data-run-steam-game="${game.appId}" ${state.steam.installed && !isRunning && !isStarting ? "" : "disabled"}>${isRunning ? "Running" : isStarting ? "Starting..." : "Play"}</button>
      </div>
    </article>
  `;
}

function step(number: string, title: string, detail: string) {
  return `
    <article class="step">
      <strong>${number}</strong>
      <div>
        <span>${title}</span>
        <small>${detail}</small>
      </div>
    </article>
  `;
}

function settingsHome() {
  return `
    <section class="panel simple-settings">
      <div class="panel-title">
        <h3>Appearance</h3>
      </div>
      <div class="theme-row">
        <div>
          <strong>Theme</strong>
          <small>${themeDetail()}</small>
        </div>
        <div class="segmented" role="group" aria-label="Theme">
          ${themeButton("auto", "Auto")}
          ${themeButton("dark", "Dark")}
          ${themeButton("light", "Light")}
        </div>
      </div>
    </section>

    <section class="panel simple-settings">
      <div class="panel-title">
        <h3>Basics</h3>
        <button data-refresh>Refresh</button>
      </div>
      ${settingsRow("Engine", state.engines[0]?.displayName ?? (engineEnsureInFlight ? "Installing…" : "Not found"), state.engines.length ? (state.engines.some((engine) => engine.d3dMetalCapable) ? "D3DMetal capable" : "Wine only — CS2 needs CrossOver/GPTK") : engineEnsureInFlight ? "Downloading Wine Stable" : "Wine installs automatically on launch")}
      ${settingsRow("Apps", `${libraryItemCount()} installed`, "Shown in Library")}
      ${settingsRow("Steam", steamSettingsValue(), "Install from the catalog")}
      ${settingsRow("Bottles", `${state.bottles.length}`, "Created automatically")}
    </section>
    ${feedbackPanel()}
    ${steamTroubleshootingPanel()}
  `;
}

function feedbackPanel() {
  return `
    <section class="panel feedback-panel">
      <div>
        <h3>Help improve Kirito</h3>
        <p>Send focused reports so compatibility data and repair paths can improve over time.</p>
      </div>
      <div class="feedback-actions">
        <button data-open-feedback="issue">Report issue</button>
        <button data-open-feedback="compatibility">App feedback</button>
        <button class="primary" data-open-feedback="feature">Feature request</button>
      </div>
    </section>
  `;
}

function themeDetail() {
  if (state.theme === "auto") return `Following macOS (${resolvedTheme()})`;
  return state.theme === "light" ? "Light with warm orange accents" : "Dark with cool blue accents";
}

function themeButton(value: ThemeMode, label: string) {
  return `<button class="${state.theme === value ? "active" : ""}" data-theme="${value}">${label}</button>`;
}

function steamTroubleshootingPanel() {
  if (!state.steam.installed) return "";

  const steamBottle = state.bottles.find((bottle) => bottle.id === state.steam.bottleId);
  const compatibilityApplied = steamBottle
    ? !steamBottle.settings.d3dMetal && !steamBottle.settings.dxvk && !steamBottle.settings.msync && !steamBottle.settings.retina
    : false;
  const vsyncEnabled = steamBottle?.settings.vsync ?? true;

  return `
    <section class="panel">
      <div class="panel-title">
        <h3>Steam troubleshooting</h3>
        <span class="pill">${vsyncEnabled ? "VSync on" : "VSync off"}</span>
      </div>
      <div class="row action-row">
        <div>
          <strong>VSync</strong>
          <small>${vsyncEnabled ? "Match the display to reduce tearing and flicker." : "Let games present frames immediately."}</small>
        </div>
        <button data-toggle-vsync="${vsyncEnabled ? "false" : "true"}">${vsyncEnabled ? "Turn off" : "Turn on"}</button>
      </div>
      <div class="row action-row">
        <div>
          <strong>Screen flicker</strong>
          <small>Switch Steam to safer graphics settings, then restart the game.</small>
        </div>
        <button data-steam-compatibility ${compatibilityApplied ? "disabled" : ""}>Reduce flicker</button>
      </div>
      <div class="row action-row">
        <div>
          <strong>Stuck Steam processes</strong>
          <small>Stop Steam, web helper, and child Wine processes in the Steam bottle.</small>
        </div>
        <button data-stop-steam ${state.steam.running ? "" : "disabled"}>Stop Steam</button>
      </div>
      <div class="row action-row">
        <div>
          <strong>Steam web cache</strong>
          <small>Clear Steam browser cache after blank store, login, or web helper issues.</small>
        </div>
        <button data-repair-steam-web-cache>Clear cache</button>
      </div>
      <div class="row action-row">
        <div>
          <strong>Controllers</strong>
          <small>${escapeHtml(controllerSettingsDetail())}</small>
        </div>
        <button data-refresh-controllers>Check again</button>
      </div>
    </section>
  `;
}

function controllerSettingsDetail() {
  const diagnostics = state.controllerDiagnostics;
  if (!diagnostics) return "Checking macOS USB and Bluetooth devices.";
  if (diagnostics.devices.length === 0) return "No controller detected by macOS.";
  return diagnostics.devices
    .slice(0, 2)
    .map((device) => `${device.name} (${device.transport})`)
    .join(", ");
}

function settingsRow(label: string, value: string, detail: string) {
  return `
    <div class="row">
      <div>
        <strong>${escapeHtml(label)}</strong>
        <small>${escapeHtml(detail)}</small>
      </div>
      <span class="pill">${escapeHtml(value)}</span>
    </div>
  `;
}

function bottleDetail(bottle: Bottle) {
  return `
    <section class="hero-panel compact">
      <div>
        <h3>${escapeHtml(bottle.windowsVersion)} bottle</h3>
        <p>${escapeHtml(bottle.path)}</p>
      </div>
      <div class="button-row">
        <button class="primary" data-install>Install App</button>
        <button data-open-bottle>Open Bottle</button>
      </div>
    </section>

    <section class="settings-grid">
      ${toggleCard("D3DMetal", "Prefer Apple's Direct3D-to-Metal path when available.", bottle.settings.d3dMetal)}
      ${toggleCard("DXVK", "Translate Direct3D 9/10/11 through Vulkan/MoltenVK.", bottle.settings.dxvk)}
      ${toggleCard("MSync", "Use synchronization support when the engine has it.", bottle.settings.msync)}
      ${toggleCard("Retina", "Expose high-DPI scaling to Windows apps.", bottle.settings.retina)}
      ${toggleCard("VSync", "Match game frames to the display.", bottle.settings.vsync)}
    </section>

    <section class="grid two">
      <article class="panel">
        <div class="panel-title">
          <h3>Installed Apps</h3>
          <button data-install>Install</button>
        </div>
        ${bottle.apps.length ? bottle.apps.map((item) => appRow(item, bottle)).join("") : empty("No launchers yet", "Install an app or add Start Menu scanning in the next milestone.")}
      </article>

      <article class="panel">
        <div class="panel-title">
          <h3>Bottle Tools</h3>
        </div>
        <div class="tool-grid">
          ${toolButton("winecfg", "Wine Config")}
          ${toolButton("regedit", "Registry")}
          ${toolButton("taskmgr", "Task Manager")}
          ${toolButton("cmd", "Command Prompt")}
        </div>
      </article>
    </section>
  `;
}

function stat(label: string, value: string, detail: string) {
  return `
    <article class="stat">
      <span>${label}</span>
      <strong>${value}</strong>
      <small>${detail}</small>
    </article>
  `;
}

function engineRow(engine: WineEngine) {
  return `
    <div class="row">
      <div>
        <strong>${escapeHtml(engine.displayName)}</strong>
        <small>${escapeHtml(engine.executable)}</small>
      </div>
      <span class="pill">${engine.d3dMetalCapable ? "D3DMetal" : "Wine"}</span>
    </div>
  `;
}

function parityRow(name: string, status: string) {
  return `
    <div class="row">
      <strong>${name}</strong>
      <span class="pill">${status}</span>
    </div>
  `;
}

function toggleCard(title: string, detail: string, enabled: boolean) {
  return `
    <article class="toggle-card ${enabled ? "enabled" : ""}">
      <div class="indicator"></div>
      <div>
        <strong>${title}</strong>
        <p>${detail}</p>
      </div>
    </article>
  `;
}

function appRow(appItem: InstalledApp, bottle: Bottle) {
  return `
    <div class="row">
      <div>
        <strong>${escapeHtml(appItem.name)}</strong>
        <small>${escapeHtml(appItem.command)}</small>
      </div>
      <button data-run-app="${appItem.id}" data-bottle="${bottle.id}">Run</button>
    </div>
  `;
}

function toolButton(command: string, label: string) {
  return `<button class="tool" data-tool="${command}">${label}</button>`;
}

function empty(title: string, detail: string) {
  return `
    <div class="empty">
      <strong>${title}</strong>
      <p>${detail}</p>
    </div>
  `;
}

function bindEvents() {
  document.querySelectorAll("[data-select-install]").forEach((button) => {
    button.addEventListener("click", () => {
      state.view = "install";
      render();
    });
  });

  document.querySelectorAll("[data-select-library]").forEach((button) => {
    button.addEventListener("click", () => {
      state.view = "library";
      render();
    });
  });

  document.querySelectorAll("[data-select-settings], [data-select-advanced]").forEach((button) => {
    button.addEventListener("click", () => {
      state.view = "settings";
      state.selectedBottleId = null;
      render();
    });
  });

  document.querySelectorAll<HTMLElement>("[data-select-bottle]").forEach((button) => {
    button.addEventListener("click", () => {
      state.view = "settings";
      state.selectedBottleId = button.dataset.selectBottle ?? null;
      render();
    });
  });

  document.querySelectorAll("[data-refresh]").forEach((button) => {
    button.addEventListener("click", () => void refresh());
  });

  document.querySelectorAll<HTMLElement>("[data-theme]").forEach((button) => {
    button.addEventListener("click", () => {
      const theme = button.dataset.theme;
      if (theme === "auto" || theme === "dark" || theme === "light") {
        saveTheme(theme);
        render();
      }
    });
  });

  document.querySelectorAll("[data-install-any-app]").forEach((button) => {
    button.addEventListener("click", () => {
      const bottle = selectedBottle() ?? state.bottles[0];
      if (!bottle) {
        void runCommand("Creating bottle", async () => {
          const created = await apiInvoke<Bottle>("create_bottle", { name: null });
          await apiInvoke("install_app", { bottleId: created.id });
        });
        return;
      }
      void runCommand("Opening installer picker", () => apiInvoke("install_app", { bottleId: bottle.id }));
    });
  });

  const installSearch = document.querySelector<HTMLInputElement>("#install-search");
  installSearch?.addEventListener("input", () => {
    if (installSearchTimer) window.clearTimeout(installSearchTimer);
    state.installQuery = installSearch.value;
    installSearchTimer = window.setTimeout(() => render(), 120);
  });

  document.querySelectorAll<HTMLElement>("[data-install-catalog]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const appId = button.dataset.installCatalog;
      if (appId === "minecraft-bedrock") {
        if (state.bedrock.installed) {
          void runLaunchCommand("Launching Minecraft Bedrock", () => apiInvoke("launch_bedrock"));
        } else {
          void runCommand("Setting up Minecraft Bedrock", () => apiInvoke("install_bedrock"));
        }
        return;
      }
      if (appId === "steam") {
        if (state.steam.installed) {
          rememberApp({ name: "Steam", subtitle: "Launcher", iconUrl: catalogApps.find((item) => item.id === "steam")?.iconUrl, action: "launch-steam" });
          void runLaunchCommand("Launching Steam", () => apiInvoke("launch_steam"));
        } else {
          void runCommand("Installing Steam", async () => {
            await apiInvoke<Bottle>("install_steam");
          });
        }
        return;
      }

      const bottle = selectedBottle() ?? state.bottles[0];
      if (!bottle) {
        void runCommand("Creating bottle", async () => {
          const created = await apiInvoke<Bottle>("create_bottle", { name: null });
          await apiInvoke("install_app", { bottleId: created.id });
        });
        return;
      }

      void runCommand("Choose the installer for this app", () => apiInvoke("install_app", { bottleId: bottle.id }));
    });
  });

  document.querySelectorAll<HTMLElement>("[data-open-catalog-detail]").forEach((row) => {
    row.addEventListener("click", () => {
      state.detailCatalogId = row.dataset.openCatalogDetail ?? null;
      render();
    });
  });

  document.querySelector("[data-back-catalog]")?.addEventListener("click", () => {
    state.detailCatalogId = null;
    render();
  });

  document.querySelectorAll("[data-install-steam]").forEach((button) => {
    button.addEventListener("click", () => {
      if (state.steam.installed) {
        rememberApp({ name: "Steam", subtitle: "Launcher", iconUrl: catalogApps.find((item) => item.id === "steam")?.iconUrl, action: "launch-steam" });
        void runLaunchCommand("Launching Steam", () => apiInvoke("launch_steam"));
      } else {
        void runCommand("Installing Steam", async () => {
          await apiInvoke<Bottle>("install_steam");
        });
      }
    });
  });

  document.querySelectorAll("[data-launch-steam]").forEach((button) => {
    button.addEventListener("click", () => {
      rememberApp({ name: "Steam", subtitle: "Launcher", iconUrl: catalogApps.find((item) => item.id === "steam")?.iconUrl, action: "launch-steam" });
      void runLaunchCommand("Launching Steam", () => apiInvoke("launch_steam"));
    });
  });

  document.querySelector("[data-refresh-steam-library]")?.addEventListener("click", () => {
    void runCommand("Refreshing Steam library", async () => {
      state.steam.games = await apiInvoke<InstalledSteamGame[]>("scan_steam_library");
      void loadAntiCheatReports(state.steam.games);
    });
  });

  document.querySelector("[data-steam-compatibility]")?.addEventListener("click", () => {
    void runCommand("Applying safer Steam graphics", async () => {
      await apiInvoke<Bottle>("apply_steam_compatibility_mode");
    });
  });

  document.querySelector("[data-stop-steam]")?.addEventListener("click", () => {
    void runCommand("Stopping Steam", async () => {
      await apiInvoke("stop_steam");
    });
  });

  document.querySelector("[data-repair-steam-web-cache]")?.addEventListener("click", () => {
    void runCommand("Clearing Steam web cache", async () => {
      await apiInvoke("repair_steam_web_cache");
    });
  });

  document.querySelectorAll<HTMLElement>("[data-open-feedback]").forEach((button) => {
    button.addEventListener("click", () => {
      void runLaunchCommand("Opening feedback", () =>
        apiInvoke("open_feedback", {
          kind: button.dataset.openFeedback ?? "feedback",
          appName: button.dataset.feedbackAppName,
          source: button.dataset.feedbackSource,
          steamAppId: optionalNumber(button.dataset.feedbackSteamAppId),
          settings: button.dataset.feedbackSettings,
          notes: button.dataset.feedbackNotes
        })
      );
    });
  });

  document.querySelector<HTMLElement>("[data-launch-failure-action]")?.addEventListener("click", (event) => {
    const button = event.currentTarget;
    if (!(button instanceof HTMLElement)) return;
    const appId = Number(button.dataset.launchFailureApp);
    const action = button.dataset.launchFailureAction;
    if (action === "clear-cache") {
      void runCommand("Clearing Steam web cache", async () => {
        await apiInvoke("repair_steam_web_cache");
      });
      return;
    }
    if (action === "stop-steam") {
      void runCommand("Stopping Steam", async () => {
        await apiInvoke("stop_steam");
      });
      return;
    }
    if (action === "settings") {
      const game = state.steam.games.find((item) => item.appId === appId);
      const bottleId = state.steam.bottleId;
      if (game && bottleId) {
        state.appSettingsTarget = {
          bottleId,
          appKey: `steam:${appId}`,
          name: game.name,
          subtitle: "Steam game"
        };
        state.view = "settings";
        render();
      }
      return;
    }
    if (action === "retry" && appId) {
      void runSteamGameLaunch(appId);
    }
  });

  document.querySelector("[data-refresh-controllers]")?.addEventListener("click", () => {
    void runCommand("Checking controllers", async () => {
      await loadControllerDiagnostics();
    });
  });

  document.querySelector<HTMLElement>("[data-toggle-vsync]")?.addEventListener("click", (event) => {
    const button = event.currentTarget;
    if (!(button instanceof HTMLElement)) return;
    const bottleId = state.steam.bottleId;
    if (!bottleId) return;
    void runCommand("Updating VSync", async () => {
      await apiInvoke<Bottle>("set_bottle_vsync", {
        bottleId,
        enabled: button.dataset.toggleVsync === "true"
      });
    });
  });

  const searchInput = document.querySelector<HTMLInputElement>("#steam-search");
  searchInput?.addEventListener("input", () => {
    if (searchTimer) window.clearTimeout(searchTimer);
    const value = searchInput.value;
    state.steamQuery = value;
    searchTimer = window.setTimeout(() => void searchSteam(value), 260);
  });

  document.querySelectorAll<HTMLElement>("[data-open-steam-game]").forEach((button) => {
    button.addEventListener("click", () => {
      void runLaunchCommand("Opening game in Steam", () =>
        apiInvoke("open_steam_game", { appId: Number(button.dataset.openSteamGame) })
      );
    });
  });

  document.querySelectorAll<HTMLElement>("[data-run-steam-game]").forEach((button) => {
    button.addEventListener("click", () => {
      const steamAppId = Number(button.dataset.runSteamGame);
      const game = state.steam.games.find((item) => item.appId === steamAppId)
        ?? state.steamResults.find((item) => item.appId === steamAppId);
      rememberApp({
        name: game?.name ?? "Steam game",
        subtitle: "Steam game",
        iconUrl: game?.image ?? undefined,
        action: "run-steam-game",
        steamAppId
      });
      void runSteamGameLaunch(steamAppId);
    });
  });

  document.querySelectorAll<HTMLElement>("[data-run-app]").forEach((button) => {
    button.addEventListener("click", () => {
      const bottleId = button.dataset.bottle;
      const appId = button.dataset.runApp;
      const bottle = state.bottles.find((item) => item.id === bottleId);
      const app = bottle?.apps.find((item) => item.id === appId);
      if (!bottle || !app) return;
      rememberApp({
        name: app.name,
        subtitle: bottle.name,
        iconUrl: catalogAppForName(app.name)?.iconUrl,
        action: "run-app",
        bottleId: bottle.id,
        appId: app.id
      });
      void runLaunchCommand(`Opening ${app.name}`, () => apiInvoke("run_app", { bottleId: bottle.id, appId: app.id }));
    });
  });

  document.querySelector("[data-open-last-used]")?.addEventListener("click", () => {
    const recent = state.lastUsedApp;
    if (!recent) return;
    if (recent.action === "launch-steam") {
      if (!state.steam.installed) {
        clearLastUsedApp();
        state.status = "Steam is not installed yet.";
        render();
        return;
      }
      void runLaunchCommand("Launching Steam", () => apiInvoke("launch_steam"));
      return;
    }
    if (recent.action === "run-steam-game" && recent.steamAppId) {
      if (!state.steam.installed) {
        clearLastUsedApp();
        state.status = "Install Steam before starting Steam games.";
        render();
        return;
      }
      void runSteamGameLaunch(recent.steamAppId, `Starting ${recent.name}`);
      return;
    }
    if (recent.action === "run-app" && recent.bottleId && recent.appId) {
      void runLaunchCommand(`Opening ${recent.name}`, () =>
        apiInvoke("run_app", { bottleId: recent.bottleId, appId: recent.appId })
      );
    }
  });

  document.querySelectorAll<HTMLElement>("[data-app-settings]").forEach((button) => {
    button.addEventListener("click", () => {
      const bottleId = button.dataset.bottle;
      const appKey = button.dataset.appSettings;
      const name = button.dataset.appName;
      if (!bottleId || !appKey || !name) return;
      state.appSettingsTarget = {
        bottleId,
        appKey,
        name,
        subtitle: button.dataset.appSubtitle ?? ""
      };
      state.view = "library";
      render();
    });
  });

  document.querySelector("[data-back-library]")?.addEventListener("click", () => {
    state.appSettingsTarget = null;
    render();
  });

  document.querySelector("[data-reset-app-settings]")?.addEventListener("click", () => {
    const target = state.appSettingsTarget;
    if (!target) return;
    void runCommand("Resetting app settings", () =>
      apiInvoke("set_app_settings", {
        bottleId: target.bottleId,
        appKey: target.appKey,
        settings: emptyAppSettings()
      })
    );
  });

  document.querySelectorAll<HTMLElement>("[data-app-preset]").forEach((button) => {
    button.addEventListener("click", () => {
      const target = state.appSettingsTarget;
      if (!target) return;
      const currentEngine = document.querySelector<HTMLSelectElement>("#app-engine")?.value || null;
      const settings = button.dataset.appPreset === "metal"
        ? {
            engineExecutable: currentEngine,
            vsync: true,
            d3dMetal: true,
            dxvk: false,
            msync: true,
            retina: false
          }
        : {
            engineExecutable: currentEngine,
            vsync: true,
            d3dMetal: false,
            dxvk: false,
            msync: false,
            retina: false
          };
      void runCommand(`Applying ${button.textContent?.trim() ?? "preset"}`, () =>
        apiInvoke("set_app_settings", {
          bottleId: target.bottleId,
          appKey: target.appKey,
          settings
        })
      );
    });
  });

  document.querySelector("[data-apply-recommended-settings]")?.addEventListener("click", () => {
    const target = state.appSettingsTarget;
    if (!target) return;
    const recommendation = appSettingsRecommendation(target);
    if (!recommendation) return;
    const currentEngine = document.querySelector<HTMLSelectElement>("#app-engine")?.value || null;
    void runCommand("Applying recommended settings", () =>
      apiInvoke("set_app_settings", {
        bottleId: target.bottleId,
        appKey: target.appKey,
        settings: {
          ...recommendation.settings,
          engineExecutable: currentEngine
        }
      })
    );
  });

  document.querySelector("[data-save-app-settings]")?.addEventListener("click", () => {
    const target = state.appSettingsTarget;
    if (!target) return;
    const engineExecutable = document.querySelector<HTMLSelectElement>("#app-engine")?.value || null;
    const settings: AppSettings = {
      engineExecutable,
      vsync: readOverride("app-vsync"),
      d3dMetal: readOverride("app-d3dmetal"),
      dxvk: readOverride("app-dxvk"),
      msync: readOverride("app-msync"),
      retina: readOverride("app-retina")
    };
    void runCommand("Saving app settings", () =>
      apiInvoke("set_app_settings", {
        bottleId: target.bottleId,
        appKey: target.appKey,
        settings
      })
    );
  });

  document.querySelectorAll("[data-create-bottle]").forEach((button) => {
    button.addEventListener("click", () => {
      void runCommand("Creating bottle", async () => {
        const bottle = await apiInvoke<Bottle>("create_bottle", { name: null });
        state.view = "settings";
        state.selectedBottleId = bottle.id;
      });
    });
  });

  document.querySelectorAll("[data-install]").forEach((button) => {
    button.addEventListener("click", () => {
      const bottle = selectedBottle();
      if (!bottle) return;
      void runCommand("Opening installer picker", () => apiInvoke("install_app", { bottleId: bottle.id }));
    });
  });

  document.querySelector("[data-open-bottle]")?.addEventListener("click", () => {
    const bottle = selectedBottle();
    if (!bottle) return;
    void runCommand("Opening bottle", () => apiInvoke("open_path", { path: bottle.path }));
  });

  document.querySelectorAll<HTMLElement>("[data-tool]").forEach((button) => {
    button.addEventListener("click", () => {
      const bottle = selectedBottle();
      if (!bottle) return;
      void runCommand(`Running ${button.dataset.tool}`, () =>
        apiInvoke("run_wine_tool", { bottleId: bottle.id, tool: button.dataset.tool })
      );
    });
  });
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (character) => {
    const replacements: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;"
    };
    return replacements[character];
  });
}

function escapeAttribute(value: string) {
  return escapeHtml(value).replace(/`/g, "&#096;");
}

function optionalNumber(value: string | undefined) {
  if (!value) return undefined;
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

window.addEventListener("focus", () => {
  void refreshSteamStatus();
});

window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => {
  if (state.theme === "auto") render();
});

window.setInterval(() => {
  if (!document.hidden) {
    void refreshSteamStatus();
  }
}, STEAM_STATUS_POLL_MS);

applyTheme();
void setupInstallProgress();
void refresh();

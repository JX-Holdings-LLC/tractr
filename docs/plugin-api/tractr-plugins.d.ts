// TypeScript typings for Tractr plugins and project scripts (docs/PLUGINS.md).
//
// A plugin's `source` is the body of `function (api) { … }`. To get completion and type checks
// while writing one in an editor, keep the source in a `.js` or `.ts` file that starts with
//
//   /// <reference path="tractr-plugins.d.ts" />
//
// and treat `api` as a `TractrApi`. `crates/farm-plugins/tests/typings.rs` keeps this file in
// step with the engine: every hook, mutation and read the engine knows is listed here.

/** Any JSON value. */
type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

// ── Hooks ──────────────────────────────────────────────────────────────────────────────────

interface HookPayloads {
  onDayStart: { day: number; season: string; year: number };
  onDayEnd: { day: number; season: string; year: number };
  onSeasonChange: { season: string; previousSeason: string; year: number };
  onYearStart: { year: number };
  onTimeOfDay: { hour: number; day: number };
  onSceneEnter: { sceneId: string; fromSceneId: string };
  onCropHarvest: { cropType: string; quantity: number; quality: string };
  onItemSell: { itemId: string; quantity: number; money: number; via: 'shop' | 'shipping'; shopId?: string };
  onRecipeCraft: { recipeId: string };
  onFishCatch: { itemId: string; fishTableId: string };
  onResourceGather: { nodeTypeId: string; drops: { itemId: string; quantity: number }[] };
  onNPCInteract: { npcId: string };
  onGiftGiven: { npcId: string; itemId: string; reaction: string };
  onRelationshipChange: { npcId: string; friendship: number };
  onBuild: { buildingId: string; sceneId: string };
  onBundleComplete: { bundleId: string };
  onAchievementUnlock: { achievementId: string };
  onMonsterSlay: { monsterId: string; sceneId: string };
  onAction: { actionId: string };
  onMinigameResolve: { minigameId: string; score: number };
  onWeatherRoll: { weatherId: string; day: number };
  onCommand: { commandType: string };
  onEffect: { effectType: string };
  onLoad: { data: Json };
}

type HookName = keyof HookPayloads;

// ── Mutations ──────────────────────────────────────────────────────────────────────────────

type Mutation =
  | { type: 'message'; text: string }
  | { type: 'playSound'; soundId: string }
  | { type: 'giveItem'; itemId: string; quantity: number }
  | { type: 'takeItem'; itemId: string; quantity: number }
  | { type: 'giveMoney'; amount: number }
  | { type: 'takeMoney'; amount: number }
  | { type: 'setFlag'; flag: string; value: boolean | number | string }
  | { type: 'modifyFriendship'; npcId: string; delta: number }
  | { type: 'grantXp'; skill: string; amount: number }
  | { type: 'modifyEnergy'; delta: number }
  | { type: 'setWeather'; weatherId: string }
  | { type: 'startQuest'; questId: string }
  | { type: 'warpPlayer'; sceneId: string; x: number; y: number }
  | { type: 'startDialogue'; npcId: string; dialogueId?: string }
  | { type: 'performAction'; actionId: string }
  | { type: 'startMinigame'; minigameId: string }
  | { type: 'moveNpc'; npcId: string; sceneId?: string; x: number; y: number }
  | { type: 'startCutscene'; cutsceneId: string }
  | { type: 'setVariable'; variable: string; value: number | string }
  | { type: 'addVariable'; variable: string; amount: number }
  | { type: 'addShopStock'; shopId: string; itemId: string; price?: number; dailyLimit?: number }
  | { type: 'removeShopStock'; shopId: string; itemId: string }
  | { type: 'placeNode'; sceneId?: string; x: number; y: number; nodeTypeId: string }
  | { type: 'removeNode'; sceneId?: string; x: number; y: number }
  | { type: 'placeMachine'; sceneId?: string; x: number; y: number; machineTypeId: string }
  | { type: 'removeMachine'; sceneId?: string; x: number; y: number }
  | { type: 'setRelationship'; npcId: string; status: 'single' | 'dating' | 'engaged' | 'married' }
  | { type: 'sendMail'; mailId: string }
  | { type: 'modifyHealth'; delta: number }
  | { type: 'setInventorySize'; amount: number }
  | { type: 'addInventorySize'; amount: number }
  | { type: 'addMaxEnergy'; amount: number }
  | { type: 'addMaxHealth'; amount: number }
  | {
      type: 'applyBuff';
      buff: {
        id: string;
        stat: string;
        amount: number;
        durationMinutes: number;
        icon?: string;
        persistsAfterSleep?: boolean;
        additive?: boolean;
      };
    }
  | { type: 'saveData'; data: Json }
  | { type: 'spawnEntity'; entityType: string; sceneId?: string; x: number; y: number; art?: string; state?: Json }
  | { type: 'updateEntity'; entityId: string; sceneId?: string; x?: number; y?: number; art?: string; state?: Json }
  | { type: 'removeEntity'; entityId: string }
  | { type: 'startTimer'; timer: string; minutes?: number; days?: number; data?: Json }
  | { type: 'cancelTimer'; timer: string };

type Answer = Mutation[] | void;

// ── The read API ───────────────────────────────────────────────────────────────────────────

interface PlayerView {
  sceneId: string;
  x: number;
  y: number;
  tileX: number;
  tileY: number;
  facing: 'up' | 'down' | 'left' | 'right';
  money: number;
  energy: number;
  maxEnergy: number;
  health?: number;
  maxHealth?: number;
  equippedTool?: string;
  name: string;
  farmName: string;
  skills: { [skill: string]: { level: number; xp: number; effectiveLevel: number } };
  worn: { itemId: string; slot: string | null }[];
  buffs: { id: string; stat: string; amount: number; source: string; expiresAtMinute: number }[];
  inventorySize: number;
}

interface DateView {
  day: number;
  season: string;
  dayOfSeason: number;
  year: number;
  minute: number;
  weather: string;
  weekday: string | null;
  festival: string | null;
  tick: number;
}

interface TileView {
  sceneId: string;
  x: number;
  y: number;
  terrain: string;
  overlay: string | null;
  object: string | null;
  collision: boolean;
  soil: string | null;
  crop: { type: string; stage: number; watered: boolean; daysGrown?: number; withered?: boolean; quality: string } | null;
  node: { typeId: string; remainingHealth: number } | null;
  machine: { typeId: string; processing: Json; ready: boolean } | null;
  entities: string[];
}

interface EntityView {
  id: string;
  typeId: string;
  sceneId: string;
  x: number;
  y: number;
  art?: string;
  state?: Json;
}

interface NpcView {
  id: string;
  sceneId: string;
  x: number;
  y: number;
  friendship: number;
  hearts: number;
  relationship: string;
}

interface QuestView {
  status: string;
  objectives: { [id: string]: { progress: number; completed: boolean } };
  deadlineDay?: number;
}

/** `api.state`: read-only queries of the game at the step the hook fired at (#237). */
interface GameState {
  /** Needs `permissions.read` `player`. */
  player(): PlayerView;
  inventory(): { itemId: string; quantity: number; quality?: string }[];
  itemCount(itemId: string): number;
  /** Needs `world`. */
  date(): DateView;
  tile(x: number, y: number, sceneId?: string): TileView | null;
  animals(): Json[];
  buildings(): Json[];
  entities(filter?: { typeId?: string; sceneId?: string }): EntityView[];
  timers(): { pluginId: string; id: string; dueAtMinute: number; data?: Json }[];
  /** Needs `people`; flags and variables are the pack's own unless `any` is granted. */
  npc(npcId: string): NpcView | null;
  npcs(): NpcView[];
  quest(questId: string): QuestView;
  quests(): { [questId: string]: QuestView };
  flag(flag: string): boolean | number | string | null;
  variable(variable: string): number | string;
  /** Needs `content`. */
  item(itemId: string): Json;
  crop(cropId: string): Json;
  definition(
    kind: 'item' | 'crop' | 'npc' | 'quest' | 'shop' | 'recipe' | 'machineType' | 'nodeType' | 'animalSpecies' | 'buildingType' | 'monster',
    id: string,
  ): Json;
}

// ── Minigames, systems and screens ─────────────────────────────────────────────────────────

interface MinigameInput {
  up: boolean;
  down: boolean;
  left: boolean;
  right: boolean;
  action: boolean;
  actionPressed: boolean;
  actionReleased: boolean;
  pointer: { x: number; y: number; down: boolean; pressed: boolean; released: boolean } | null;
}

type DrawCommand =
  | { type: 'rect'; x: number; y: number; w: number; h: number; color?: string; radius?: number; outline?: number }
  | { type: 'text'; x: number; y: number; text: string; size?: number; color?: string; align?: 'left' | 'center' | 'right'; bold?: boolean }
  | { type: 'sprite'; x: number; y: number; w: number; h: number; item: string };

interface Minigame<S extends Json> {
  start?(config: Json, info: { width: number; height: number; seed: number }): S;
  update(state: S, input: MinigameInput, dt: number): { state?: S; done?: { score: number } | { success: boolean }; sounds?: string[] };
  draw(state: S): DrawCommand[];
}

interface EntityHandlers {
  onTick?(entity: EntityView, state: GameState, call: { op: 'tick'; minute: number }): Answer;
  onInteract?(entity: EntityView, state: GameState): Answer;
  onDayStart?(entity: EntityView, state: GameState, call: { op: 'dayStart'; day: number }): Answer;
}

type Widget =
  | { type: 'text'; text: string; muted?: boolean; size?: 'small' | 'normal' | 'large' }
  | { type: 'heading'; text: string }
  | { type: 'column'; children: Widget[] }
  | { type: 'row'; children: Widget[] }
  | { type: 'list'; items: { text: string; detail?: string; icon?: string; action?: string }[] }
  | { type: 'slots'; slots: { itemId?: string; quantity?: number; action?: string }[]; columns?: number }
  | { type: 'tabs'; tabs: { id: string; label: string }[]; selected: string }
  | { type: 'button'; label: string; action: string; primary?: boolean; enabled?: boolean }
  | { type: 'item'; itemId: string; quantity?: number }
  | { type: 'portrait'; npcId: string }
  | { type: 'progress'; value: number; label?: string };

interface Screen {
  title?: string;
  hotkey?: string;
  draw(state: GameState): Widget;
  action?(action: string, state: GameState): Answer;
}

interface TractrApi {
  on<H extends HookName>(hook: H, handler: (payload: HookPayloads[H]) => Answer): void;
  /** The raw host call behind `state` (guest ABI 2). */
  host(request: Json): Json;
  readonly state: GameState;
  minigame<S extends Json>(name: string, game: Minigame<S>): void;
  condition(name: string, fn: (args: Json, state: GameState, context: { npcId: string | null; x: number | null; y: number | null }) => boolean): void;
  outcome(name: string, fn: (args: Json, state: GameState) => Answer): void;
  entity(name: string, handlers: EntityHandlers): void;
  itemUse(name: string, fn: (use: { itemId: string; sceneId: string; x: number; y: number }, state: GameState) => Answer): void;
  timer(name: string, fn: (timer: { timer: string; data: Json }, state: GameState) => Answer): void;
  screen(name: string, screen: Screen): void;
}

declare const api: TractrApi;

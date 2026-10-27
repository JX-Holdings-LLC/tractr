using System.Globalization;

namespace Tractr.App.Localization;

/// <summary>A language of the editor's chrome: its code and its name in itself.</summary>
public sealed record EditorLanguage(string Code, string NativeName);

/// <summary>
/// String tables of the editor's own chrome: mode
/// names, the Play Mode toolbar, the whole menu bar, the welcome tour and the shortcuts list,
/// and the Edit Mode controls M1 and M2 added (the map's sky preview and brushes, a scene's
/// surroundings, sounds and music, the game's words, weather overlays and sky). The game's
/// interface is localized by the Rust player (farm-ui); the rest of the editor's tabs, forms and
/// Problems stay English until the full editor pass (ROADMAP "Later"). Edit Mode reads its
/// strings when it is built, so a language change shows there once a project is opened again.
/// <para>
/// <see cref="Get(string)"/> looks a key up in the current <see cref="Language"/>, falls back to
/// English, then to the key itself. The language is an app setting
/// (<see cref="Projects.WorkspaceSettings.EditorLanguage"/>, Help → Language); without one the
/// app starts in the system's language when there is a table for it.
/// </para>
/// </summary>
public static class EditorStrings
{
    public const string English = "en";

    public static readonly IReadOnlyList<EditorLanguage> Languages =
    [
        new(English, "English"),
        new("es", "Español"),
    ];

    private static readonly Dictionary<string, string> En = new(StringComparer.Ordinal)
    {
        // Web i18n.ts `mode.*` and `toolbar.*`.
        ["mode.play"] = "Play Mode",
        ["mode.edit"] = "Edit Mode",
        ["mode.projects"] = "Projects",
        ["toolbar.restart"] = "Restart",
        ["toolbar.keepChanges"] = "Keep changes",
        ["toolbar.debug"] = "Debug",
        ["toolbar.restartTip"] = "Restore the pre-playtest snapshot and start over",
        ["toolbar.keepChangesTip"] = "Keep playtest changes when exiting to the editor",
        ["toolbar.debugTip"] = "Playtest debug drawer",
        ["toolbar.playHint"] = "Click the game to play. Esc pauses; F6 returns to the editor.",
        ["toolbar.keepOn"] = "Playtest changes will be kept when you exit",
        ["toolbar.keepOff"] = "Playtest changes will be discarded when you exit",
        // Header.
        ["header.playing"] = "Playing: {0}",
        ["header.editing"] = "Editing: {0}",
        // Menus (mnemonics with "_").
        ["menu.file"] = "_File",
        ["menu.newProject"] = "_New Project",
        ["menu.openProject"] = "_Open Project…",
        ["menu.importJson"] = "_Import Project JSON…",
        ["menu.exportJson"] = "_Export Project JSON…",
        ["menu.exportGame"] = "Export _Game…",
        ["menu.exit"] = "E_xit",
        ["menu.game"] = "_Game",
        ["menu.playMode"] = "_Play Mode",
        ["menu.editMode"] = "_Edit Mode",
        ["menu.help"] = "_Help",
        ["menu.welcome"] = "_Welcome Tour",
        ["menu.creatorGuide"] = "_Creator Guide",
        ["menu.shortcuts"] = "_Keyboard Shortcuts",
        ["menu.language"] = "_Language",
        ["menu.updateCenter"] = "_Update Center…",
        ["menu.about"] = "_About Tractr",
        // Welcome tour (web WelcomeDialog, adapted to the desktop editor).
        ["welcome.title"] = "Welcome to Tractr",
        ["welcome.subtitle"] = "Build and share a farming adventure — no code required.",
        ["welcome.play.title"] = "Play the starter farm",
        ["welcome.play.body"] = "Press F5 (or Play Mode) to playtest. Move with WASD or the arrow keys, use tools with T (hoe) and Q (water), interact with E and sleep with Z. Esc opens the game's menu; F6 returns to the editor.",
        ["welcome.edit.title"] = "Build in Edit Mode",
        ["welcome.edit.body"] = "Paint terrain with the map tools (Brush, Rectangle, Fill area), place NPCs, nodes and items, and add doors between scenes. Undo anything with Ctrl+Z.",
        ["welcome.content.title"] = "Make it yours",
        ["welcome.content.body"] = "Add crops, NPCs with dialogue, quests, shops and events in the Content tab, start from a Workshop pattern, and import art in the Art tab. Changes made while playing are discarded when you return, unless you turn on Keep changes.",
        ["welcome.share.title"] = "Share your game",
        ["welcome.share.body"] = "File → Export Game (Ctrl+Shift+E) builds standalone games for Windows and Linux (Steam Deck too) that anyone can play without the editor.",
        ["welcome.guide"] = "Full creator guide",
        ["welcome.start"] = "Let's farm!",
        // Help windows.
        ["guide.title"] = "Creator Guide",
        ["shortcuts.title"] = "Keyboard Shortcuts",
        ["shortcuts.editor"] = "EDITOR",
        ["shortcuts.game"] = "IN THE GAME (DEFAULT KEYS)",
        ["shortcuts.newProject"] = "New project",
        ["shortcuts.openProject"] = "Open project",
        ["shortcuts.exportGame"] = "Export Game",
        ["shortcuts.playMode"] = "Play Mode (playtest)",
        ["shortcuts.editMode"] = "Back to Edit Mode",
        ["shortcuts.undo"] = "Undo (Edit Mode)",
        ["shortcuts.redo"] = "Redo (Edit Mode)",
        ["shortcuts.mapCursor"] = "Move the map's editing cursor (map focused)",
        ["shortcuts.mapApply"] = "Use the map tool at the cursor",
        ["shortcuts.mapCancel"] = "Cancel a rectangle's first corner",
        ["shortcuts.mapTools"] = "Map tools (map focused): Inspect, Brush, Rectangle, Fill, Pick, Select, Erase, Block, Unblock, Door, Player start",
        ["shortcuts.quickOpen"] = "Find content, scenes and art (Edit Mode)",
        ["shortcuts.playToolbar"] = "Play Mode: Restart · Keep changes · Debug",
        ["shortcuts.exit"] = "Quit the editor",
        ["shortcuts.guide"] = "Creator guide",
        ["shortcuts.move"] = "Move",
        ["shortcuts.interact"] = "Interact, talk, confirm",
        ["shortcuts.tools"] = "Watering can · hoe · axe · pickaxe · scythe",
        ["shortcuts.craft"] = "Crafting",
        ["shortcuts.sleep"] = "Sleep",
        ["shortcuts.inventory"] = "Inventory",
        ["shortcuts.quests"] = "Quests",
        ["shortcuts.menu"] = "Close a panel; pause menu",
        ["shortcuts.options"] = "Pick a dialogue option",
        ["shortcuts.rebind"] = "Players can rebind the game's keys in Settings → Controls.",
        ["common.close"] = "Close",
        // Edit Mode: the map's sky preview.
        ["sky.label"] = "Sky",
        ["sky.name"] = "Sky preview",
        ["sky.time"] = "Time",
        ["sky.season"] = "Season",
        ["sky.weather"] = "Weather",
        ["sky.note"] = "A preview only: not saved.",
        ["sky.morning"] = "Morning",
        ["sky.noon"] = "Noon",
        ["sky.evening"] = "Evening",
        ["sky.night"] = "Night",
        ["sky.clear"] = "Clear",
        ["sky.timeName"] = "Sky preview: time of day",
        ["sky.seasonName"] = "Sky preview season",
        ["sky.weatherName"] = "Sky preview weather",
        ["sky.tip"] = "Preview the scene's day/night tint, foliage tint and weather as the game draws them, at a time, season and weather you pick. Not saved with the project.",
        ["sky.indoor"] = "An indoor scene keeps the weather out.",
        // Edit Mode (M1 and M2): the Map tab's brushes and the scene's surroundings.
        ["palette.inspect"] = "Inspect",
        ["palette.inspectName"] = "Inspect (no brush)",
        ["palette.brushName"] = "Brush: {0}",
        ["scene.dark"] = "Dark (no daylight for crops)",
        ["scene.darkTip"] = "Caves and cellars: crops that need light do not grow here, and crops that need the dark do.",
        ["scene.seasons"] = "season's",
        ["scene.temperature"] = "Temperature",
        ["scene.temperatureName"] = "Scene temperature",
        ["scene.temperatureTip"] = "How warm it is here, for crops with a temperature range; empty uses the season's.",
        ["scene.temperatureNumber"] = "Temperature must be a number.",
        ["scene.tags"] = "Tags",
        ["scene.tagsName"] = "Scene tags",
        ["scene.tagsPlaceholder"] = "cave, greenhouse",
        ["scene.tagsTip"] = "Words crops can require, separated by commas.",
        ["scene.musicName"] = "Scene music",
        ["scene.foliageName"] = "Scene foliage tint",
        ["scene.daylight"] = "Daylight (minute and colour per line)",
        ["scene.daylightName"] = "Scene daylight",
        ["scene.daylightTip"] = "This scene's own day/night tint: one line per key, the minute of the day and a colour (360 #ffd8a0). Empty uses the season's.",
        ["scene.saveSurroundings"] = "Save surroundings",
        ["scene.savedSurroundings"] = "Saved the surroundings of {0}.",
        ["scene.couldNotSaveSurroundings"] = "Could not save the surroundings: {0}",
        // Edit Mode (M1 and M2): the Art tab's sounds and music.
        ["art.sounds"] = "SOUNDS AND MUSIC",
        ["art.soundsName"] = "Sounds and music",
        ["art.soundsHelp"] = "Import OGG or WAV files. Sounds play from events, actions and cutscenes; music plays in scenes, seasons, weather and festivals.",
        ["art.audioFiles"] = "OGG or WAV audio",
        ["art.importSounds"] = "Import sounds",
        ["art.importSoundsTip"] = "Choose sound effects (OGG or WAV); they are added as one undo step.",
        ["art.importMusic"] = "Import music",
        ["art.importMusicTip"] = "Choose music tracks (OGG or WAV); they are added as one undo step.",
        ["art.removeSound"] = "Remove sound",
        ["art.removeSoundTip"] = "Remove the selected sound or music; what played it plays nothing (Problems lists those uses)",
        ["art.noSounds"] = "No sounds or music yet.",
        ["art.sound"] = "sound",
        ["art.music"] = "music",
        ["art.importedSound"] = "Imported {0} as a sound ({1}).",
        ["art.importedMusic"] = "Imported {0} as music ({1}).",
        ["art.importedSounds"] = "Imported {0} sounds.",
        ["art.importedMusicTracks"] = "Imported {0} music tracks.",
        ["art.couldNotImport"] = "Could not import: {0}",
        ["art.couldNotImportFiles"] = "Could not import {0}",
        // Edit Mode (M1 and M2): Settings' words, weather overlays and sky.
        ["settings.words"] = "WORDS",
        ["settings.wordsHelp"] = "What the game calls money, energy and the skills. Empty fields keep the built-in words.",
        ["settings.skillNames"] = "Skill names",
        ["settings.seasonsHelp"] = "Temperature is for crops with a temperature range; music plays all season instead of the game's own.",
        ["settings.temperaturePlaceholder"] = "temp.",
        ["settings.seasonTemperatureTip"] = "How warm the season is, for crops with a temperature range",
        ["settings.overlays"] = "WEATHER OVERLAYS",
        ["settings.overlaysHelp"] = "How a weather looks: rain streaks, snow flakes, drifting dust or a tint alone, beside the built-in rain and snow.",
        ["settings.addOverlay"] = "Add overlay",
        ["settings.noTint"] = "no tint",
        ["settings.densityTip"] = "Particles, relative to the built-in amount (1)",
        ["settings.speedTip"] = "Speed, relative to the built-in style (1)",
        ["settings.tintTip"] = "A colour multiplied over the scene while it falls",
        ["settings.weatherLooks"] = "Each weather's overlay and music (outdoors)",
        ["settings.overlay"] = "Overlay",
        ["settings.sky"] = "THE SKY",
        ["settings.skyHelp"] = "The day/night tint (one line per key: the minute of the day and a colour, like 360 #ffd8a0) and the tint of trees and weeds. A season's own look replaces the look of all seasons; empty fields keep the look below them, down to the built-in one.",
        ["settings.builtinSky"] = "built-in day and night",
        ["settings.allSeasonsSky"] = "the look of all seasons",
        ["settings.saveSky"] = "Save atmosphere",
        ["settings.revertSky"] = "Revert atmosphere",
        ["common.remove"] = "Remove",
        ["common.music"] = "Music",
        ["common.daylight"] = "Daylight",
        ["common.foliageTint"] = "Foliage tint",
        ["common.noChanges"] = "No changes were made.",
        // M5–M7: the farm, combat, the mine's depth bands, greenhouses and scene monsters.
        ["content.placedBuildingNote"] = "{0} at {1} ({2}, {3})",
        ["settings.farm"] = "FARM",
        ["settings.farmHelp"] = "The farm scene buildings go in by default, the farm maps a new game picks from (two or more make the character creator ask) and the chance each night that pests eat an outdoor crop no scarecrow protects. Buildings are their own content type; builders are shops that list them.",
        ["settings.farmOn"] = "Farm settings: farm scene, layouts and pests",
        ["settings.saveFarm"] = "Save farm",
        ["settings.farmWhat"] = "Farm settings",
        ["settings.combat"] = "COMBAT",
        ["settings.combatHelp"] = "Player health, fainting and monsters. Off, the player has no health and no monster appears. Monsters are their own content type; scenes and the mine's depth bands spawn them, and a tool with the attack action makes an item a weapon.",
        ["settings.combatOn"] = "Combat: player health, monsters and fainting",
        ["settings.saveCombat"] = "Save combat",
        ["settings.combatWhat"] = "Combat",
        ["settings.couldNotSave"] = "Could not save: {0}",
        ["settings.mineBands"] = "Depth bands",
        ["settings.mineBandsHelp"] = "Each band of floors has its rocks and, with combat on, its monsters, hazard terrains and treasure.",
        ["settings.mineBand"] = "Band {0}",
        ["settings.addMineBand"] = "Add band",
        ["settings.saveMineBands"] = "Save depth bands",
        ["settings.mineBandsWhat"] = "Mine depth bands",
        ["scene.allSeasons"] = "All seasons (greenhouse)",
        ["scene.allSeasonsTip"] = "Crops are planted and grow here in every season.",
        ["scene.monsters"] = "Monsters each morning (one per line)",
        ["scene.monstersName"] = "Scene monsters",
        ["scene.monstersTip"] = "With combat on: a monster id and how many, and optionally where (slime 3, bat 2 at 4,4, slime 3 in 2,2-10,8). Saved with the surroundings.",
        ["scene.monstersPlaceholder"] = "slime 3 in 2,2-10,8",
        ["scene.layers"] = "Extra layers (one per line)",
        ["scene.layersName"] = "Scene layers",
        ["scene.layersTip"] = "Layers beyond background, overlay and object, painted with terrains from the Layer list: an id, then above for one drawn over the player (roofs, treetops). Scenery: they never block. Saved with the surroundings.",
        ["scene.layersPlaceholder"] = "treetops above",
        ["map.extraLayer"] = "Layer: {0}",
    };

    private static readonly Dictionary<string, string> Es = new(StringComparer.Ordinal)
    {
        ["mode.play"] = "Modo Juego",
        ["mode.edit"] = "Modo Editor",
        ["mode.projects"] = "Proyectos",
        ["toolbar.restart"] = "Reiniciar",
        ["toolbar.keepChanges"] = "Conservar cambios",
        ["toolbar.debug"] = "Depurar",
        ["toolbar.restartTip"] = "Restaura el estado previo a la prueba y empieza de nuevo",
        ["toolbar.keepChangesTip"] = "Conserva los cambios de la prueba al volver al editor",
        ["toolbar.debugTip"] = "Panel de depuración de la prueba",
        ["toolbar.playHint"] = "Haz clic en el juego para jugar. Esc pausa; F6 vuelve al editor.",
        ["toolbar.keepOn"] = "Los cambios de la prueba se conservarán al salir",
        ["toolbar.keepOff"] = "Los cambios de la prueba se descartarán al salir",
        ["header.playing"] = "Jugando: {0}",
        ["header.editing"] = "Editando: {0}",
        ["menu.file"] = "_Archivo",
        ["menu.newProject"] = "_Nuevo proyecto",
        ["menu.openProject"] = "_Abrir proyecto…",
        ["menu.importJson"] = "_Importar JSON del proyecto…",
        ["menu.exportJson"] = "_Exportar JSON del proyecto…",
        ["menu.exportGame"] = "Exportar _juego…",
        ["menu.exit"] = "_Salir",
        ["menu.game"] = "_Juego",
        ["menu.playMode"] = "Modo _Juego",
        ["menu.editMode"] = "Modo _Editor",
        ["menu.help"] = "Ay_uda",
        ["menu.welcome"] = "_Visita de bienvenida",
        ["menu.creatorGuide"] = "_Guía del creador",
        ["menu.shortcuts"] = "_Atajos de teclado",
        ["menu.language"] = "_Idioma",
        ["menu.updateCenter"] = "_Centro de actualizaciones…",
        ["menu.about"] = "Acerca _de Tractr",
        ["welcome.title"] = "Te damos la bienvenida a Tractr",
        ["welcome.subtitle"] = "Crea y comparte una aventura en la granja, sin escribir código.",
        ["welcome.play.title"] = "Juega la granja inicial",
        ["welcome.play.body"] = "Pulsa F5 (o Modo Juego) para probar el juego. Muévete con WASD o las flechas, usa las herramientas con T (azada) y Q (regadera), interactúa con E y duerme con Z. Esc abre el menú del juego; F6 vuelve al editor.",
        ["welcome.edit.title"] = "Construye en el Modo Editor",
        ["welcome.edit.body"] = "Pinta el terreno con las herramientas del mapa (pincel, rectángulo, relleno), coloca personajes, recursos y objetos, y añade puertas entre escenas. Deshaz cualquier cambio con Ctrl+Z.",
        ["welcome.content.title"] = "Hazlo tuyo",
        ["welcome.content.body"] = "Añade cultivos, personajes con diálogos, misiones, tiendas y eventos en la pestaña Content, parte de un patrón del Workshop e importa arte en la pestaña Art. Los cambios hechos al jugar se descartan al volver, salvo que actives Conservar cambios.",
        ["welcome.share.title"] = "Comparte tu juego",
        ["welcome.share.body"] = "File → Export Game (Ctrl+Shift+E) crea juegos independientes para Windows y Linux (también Steam Deck) que cualquiera puede jugar sin el editor.",
        ["welcome.guide"] = "Guía completa del creador",
        ["welcome.start"] = "¡A cultivar!",
        ["guide.title"] = "Guía del creador",
        ["shortcuts.title"] = "Atajos de teclado",
        ["shortcuts.editor"] = "EDITOR",
        ["shortcuts.game"] = "EN EL JUEGO (TECLAS PREDETERMINADAS)",
        ["shortcuts.newProject"] = "Proyecto nuevo",
        ["shortcuts.openProject"] = "Abrir un proyecto",
        ["shortcuts.exportGame"] = "Exportar el juego",
        ["shortcuts.playMode"] = "Modo Juego (probar)",
        ["shortcuts.editMode"] = "Volver al Modo Editor",
        ["shortcuts.undo"] = "Deshacer (Modo Editor)",
        ["shortcuts.redo"] = "Rehacer (Modo Editor)",
        ["shortcuts.mapCursor"] = "Mover el cursor de edición del mapa (con el mapa enfocado)",
        ["shortcuts.mapApply"] = "Usar la herramienta del mapa en el cursor",
        ["shortcuts.mapCancel"] = "Cancelar la primera esquina de un rectángulo",
        ["shortcuts.mapTools"] = "Herramientas del mapa (con el mapa enfocado): Inspeccionar, Pincel, Rectángulo, Rellenar, Tomar, Seleccionar, Borrar, Bloquear, Desbloquear, Puerta, Inicio del jugador",
        ["shortcuts.quickOpen"] = "Buscar contenido, escenas y arte (Modo Editor)",
        ["shortcuts.playToolbar"] = "Modo Juego: Reiniciar · Conservar cambios · Depurar",
        ["shortcuts.exit"] = "Salir del editor",
        ["shortcuts.guide"] = "Guía del creador",
        ["shortcuts.move"] = "Moverse",
        ["shortcuts.interact"] = "Interactuar, hablar, confirmar",
        ["shortcuts.tools"] = "Regadera · azada · hacha · pico · guadaña",
        ["shortcuts.craft"] = "Fabricación",
        ["shortcuts.sleep"] = "Dormir",
        ["shortcuts.inventory"] = "Inventario",
        ["shortcuts.quests"] = "Misiones",
        ["shortcuts.menu"] = "Cerrar un panel; menú de pausa",
        ["shortcuts.options"] = "Elegir una opción de diálogo",
        ["shortcuts.rebind"] = "Los jugadores pueden reasignar las teclas del juego en Ajustes → Controles.",
        ["common.close"] = "Cerrar",
        // Edit Mode: the map's sky preview.
        ["sky.label"] = "Cielo",
        ["sky.name"] = "Vista previa del cielo",
        ["sky.time"] = "Hora",
        ["sky.season"] = "Estación",
        ["sky.weather"] = "Tiempo",
        ["sky.note"] = "Solo una vista previa: no se guarda.",
        ["sky.morning"] = "Mañana",
        ["sky.noon"] = "Mediodía",
        ["sky.evening"] = "Atardecer",
        ["sky.night"] = "Noche",
        ["sky.clear"] = "Despejado",
        ["sky.timeName"] = "Vista previa del cielo: hora del día",
        ["sky.seasonName"] = "Estación de la vista previa del cielo",
        ["sky.weatherName"] = "Tiempo de la vista previa del cielo",
        ["sky.tip"] = "Muestra el tinte de día y noche, el tinte del follaje y el tiempo de la escena como los dibuja el juego, a la hora, estación y tiempo que elijas. No se guarda en el proyecto.",
        ["sky.indoor"] = "Una escena interior no tiene tiempo atmosférico.",
        // Edit Mode (M1 and M2): the Map tab's brushes and the scene's surroundings.
        ["palette.inspect"] = "Inspeccionar",
        ["palette.inspectName"] = "Inspeccionar (sin pincel)",
        ["palette.brushName"] = "Pincel: {0}",
        ["scene.dark"] = "Oscura (sin luz de día para los cultivos)",
        ["scene.darkTip"] = "Cuevas y sótanos: aquí no crecen los cultivos que necesitan luz, y sí los que necesitan oscuridad.",
        ["scene.seasons"] = "la estación",
        ["scene.temperature"] = "Temperatura",
        ["scene.temperatureName"] = "Temperatura de la escena",
        ["scene.temperatureTip"] = "Cuánto calor hace aquí, para los cultivos con un rango de temperatura; vacío usa la de la estación.",
        ["scene.temperatureNumber"] = "La temperatura debe ser un número.",
        ["scene.tags"] = "Etiquetas",
        ["scene.tagsName"] = "Etiquetas de la escena",
        ["scene.tagsPlaceholder"] = "cueva, invernadero",
        ["scene.tagsTip"] = "Palabras que los cultivos pueden exigir, separadas por comas.",
        ["scene.musicName"] = "Música de la escena",
        ["scene.foliageName"] = "Tinte del follaje de la escena",
        ["scene.daylight"] = "Luz del día (minuto y color por línea)",
        ["scene.daylightName"] = "Luz del día de la escena",
        ["scene.daylightTip"] = "El tinte de día y noche propio de esta escena: una línea por clave, el minuto del día y un color (360 #ffd8a0). Vacío usa el de la estación.",
        ["scene.saveSurroundings"] = "Guardar entorno",
        ["scene.savedSurroundings"] = "Se guardó el entorno de {0}.",
        ["scene.couldNotSaveSurroundings"] = "No se pudo guardar el entorno: {0}",
        // Edit Mode (M1 and M2): the Art tab's sounds and music.
        ["art.sounds"] = "SONIDOS Y MÚSICA",
        ["art.soundsName"] = "Sonidos y música",
        ["art.soundsHelp"] = "Importa archivos OGG o WAV. Los sonidos suenan en eventos, acciones y escenas cinemáticas; la música, en escenas, estaciones, el tiempo y festivales.",
        ["art.audioFiles"] = "Audio OGG o WAV",
        ["art.importSounds"] = "Importar sonidos",
        ["art.importSoundsTip"] = "Elige efectos de sonido (OGG o WAV); se añaden en un solo paso de deshacer.",
        ["art.importMusic"] = "Importar música",
        ["art.importMusicTip"] = "Elige pistas de música (OGG o WAV); se añaden en un solo paso de deshacer.",
        ["art.removeSound"] = "Quitar sonido",
        ["art.removeSoundTip"] = "Quita el sonido o la música elegidos; lo que los reproducía no reproduce nada (Problemas lista esos usos)",
        ["art.noSounds"] = "Aún no hay sonidos ni música.",
        ["art.sound"] = "sonido",
        ["art.music"] = "música",
        ["art.importedSound"] = "Se importó {0} como sonido ({1}).",
        ["art.importedMusic"] = "Se importó {0} como música ({1}).",
        ["art.importedSounds"] = "Se importaron {0} sonidos.",
        ["art.importedMusicTracks"] = "Se importaron {0} pistas de música.",
        ["art.couldNotImport"] = "No se pudo importar: {0}",
        ["art.couldNotImportFiles"] = "No se pudo importar {0}",
        // Edit Mode (M1 and M2): Settings' words, weather overlays and sky.
        ["settings.words"] = "PALABRAS",
        ["settings.wordsHelp"] = "Cómo llama el juego al dinero, la energía y las habilidades. Los campos vacíos conservan las palabras integradas.",
        ["settings.skillNames"] = "Nombres de las habilidades",
        ["settings.seasonsHelp"] = "La temperatura es para los cultivos con un rango de temperatura; la música suena toda la estación en lugar de la del juego.",
        ["settings.temperaturePlaceholder"] = "temp.",
        ["settings.seasonTemperatureTip"] = "Cuánto calor hace en la estación, para los cultivos con un rango de temperatura",
        ["settings.overlays"] = "CAPAS DEL TIEMPO",
        ["settings.overlaysHelp"] = "Cómo se ve un tiempo: trazos de lluvia, copos de nieve, polvo a la deriva o solo un tinte, además de la lluvia y la nieve integradas.",
        ["settings.addOverlay"] = "Añadir capa",
        ["settings.noTint"] = "sin tinte",
        ["settings.densityTip"] = "Partículas, respecto a la cantidad integrada (1)",
        ["settings.speedTip"] = "Velocidad, respecto al estilo integrado (1)",
        ["settings.tintTip"] = "Un color multiplicado sobre la escena mientras cae",
        ["settings.weatherLooks"] = "La capa y la música de cada tiempo (al aire libre)",
        ["settings.overlay"] = "Capa",
        ["settings.sky"] = "EL CIELO",
        ["settings.skyHelp"] = "El tinte de día y noche (una línea por clave: el minuto del día y un color, como 360 #ffd8a0) y el tinte de árboles y malezas. El aspecto propio de una estación sustituye al de todas las estaciones; los campos vacíos conservan el aspecto de debajo, hasta el integrado.",
        ["settings.builtinSky"] = "día y noche integrados",
        ["settings.allSeasonsSky"] = "el aspecto de todas las estaciones",
        ["settings.saveSky"] = "Guardar atmósfera",
        ["settings.revertSky"] = "Revertir atmósfera",
        ["common.remove"] = "Quitar",
        ["common.music"] = "Música",
        ["common.daylight"] = "Luz del día",
        ["common.foliageTint"] = "Tinte del follaje",
        ["common.noChanges"] = "No se hizo ningún cambio.",
        // M5–M7: la granja, el combate, las franjas de la mina, los invernaderos y los monstruos de una escena.
        ["content.placedBuildingNote"] = "{0} en {1} ({2}, {3})",
        ["settings.farm"] = "GRANJA",
        ["settings.farmHelp"] = "La escena de la granja, donde van los edificios por omisión, los mapas de granja entre los que elige una partida nueva (con dos o más, el creador de personaje pregunta) y la probabilidad de que cada noche las plagas se coman un cultivo al aire libre que ningún espantapájaros protege. Los edificios son su propio tipo de contenido; los constructores son tiendas que los ofrecen.",
        ["settings.farmOn"] = "Ajustes de la granja: escena, mapas y plagas",
        ["settings.saveFarm"] = "Guardar granja",
        ["settings.farmWhat"] = "Ajustes de la granja",
        ["settings.combat"] = "COMBATE",
        ["settings.combatHelp"] = "La salud del jugador, los desmayos y los monstruos. Apagado, el jugador no tiene salud y no aparece ningún monstruo. Los monstruos son su propio tipo de contenido; las escenas y las franjas de la mina los generan, y una herramienta con la acción de atacar hace de un objeto un arma.",
        ["settings.combatOn"] = "Combate: salud del jugador, monstruos y desmayos",
        ["settings.saveCombat"] = "Guardar combate",
        ["settings.combatWhat"] = "Combate",
        ["settings.couldNotSave"] = "No se pudo guardar: {0}",
        ["settings.mineBands"] = "Franjas de profundidad",
        ["settings.mineBandsHelp"] = "Cada franja de pisos tiene sus rocas y, con el combate encendido, sus monstruos, terrenos peligrosos y tesoros.",
        ["settings.mineBand"] = "Franja {0}",
        ["settings.addMineBand"] = "Añadir franja",
        ["settings.saveMineBands"] = "Guardar franjas",
        ["settings.mineBandsWhat"] = "Franjas de la mina",
        ["scene.allSeasons"] = "Todas las estaciones (invernadero)",
        ["scene.allSeasonsTip"] = "Aquí los cultivos se plantan y crecen en cualquier estación.",
        ["scene.monsters"] = "Monstruos cada mañana (uno por línea)",
        ["scene.monstersName"] = "Monstruos de la escena",
        ["scene.monstersTip"] = "Con el combate encendido: el id de un monstruo y cuántos, y si se quiere dónde (slime 3, bat 2 at 4,4, slime 3 in 2,2-10,8). Se guarda con el entorno.",
        ["scene.monstersPlaceholder"] = "slime 3 in 2,2-10,8",
        ["scene.layers"] = "Capas extra (una por línea)",
        ["scene.layersName"] = "Capas de la escena",
        ["scene.layersTip"] = "Capas además del fondo, la superposición y el objeto, pintadas con terrenos de la lista Capa: un id, y above para una que se dibuja sobre el jugador (tejados, copas de árboles). Son decorado: nunca bloquean. Se guarda con el entorno.",
        ["scene.layersPlaceholder"] = "treetops above",
        ["map.extraLayer"] = "Capa: {0}",
    };

    private static readonly Dictionary<string, Dictionary<string, string>> Tables = new(StringComparer.Ordinal)
    {
        [English] = En,
        ["es"] = Es,
    };

    /// <summary>The current language's code.</summary>
    public static string Language { get; private set; } = English;

    /// <summary>Raised after <see cref="Language"/> changes.</summary>
    public static event EventHandler? LanguageChanged;

    /// <summary>
    /// Switches the language (<paramref name="code"/> may be any tag, like <c>es-MX</c>; one without
    /// a table means English).
    /// </summary>
    public static void SetLanguage(string? code)
    {
        var language = FromTag(code) ?? English;
        if (language == Language)
        {
            return;
        }

        Language = language;
        LanguageChanged?.Invoke(null, EventArgs.Empty);
    }

    /// <summary>The language with a table for a locale tag (<c>es</c>, <c>es-MX</c>, <c>es_ES.UTF-8</c>).</summary>
    public static string? FromTag(string? tag)
    {
        if (string.IsNullOrWhiteSpace(tag))
        {
            return null;
        }

        var language = new string(tag.Trim().TakeWhile(char.IsAsciiLetter).ToArray()).ToLowerInvariant();
        return Tables.ContainsKey(language) ? language : null;
    }

    /// <summary>The creator's choice, else the system's language, else English.</summary>
    public static string Resolve(string? preference, string? system) => FromTag(preference) ?? FromTag(system) ?? English;

    /// <summary>The app's language at startup: the saved choice, else the system's language.</summary>
    public static string ResolveForSystem(string? preference) => Resolve(preference, SystemLocale());

    /// <summary>
    /// The system's locale tag. The app runs with invariant globalization, so this reads the
    /// user's locale setting directly: <c>LC_ALL</c>, <c>LC_MESSAGES</c>, <c>LANG</c> on Linux and
    /// macOS, the user's locale name in the registry on Windows.
    /// </summary>
    public static string? SystemLocale()
    {
        if (OperatingSystem.IsWindows())
        {
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Control Panel\International");
                return key?.GetValue("LocaleName") as string;
            }
            catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
            {
                return null;
            }
        }

        return new[] { "LC_ALL", "LC_MESSAGES", "LANG" }
            .Select(Environment.GetEnvironmentVariable)
            .FirstOrDefault(value => !string.IsNullOrWhiteSpace(value));
    }

    /// <summary>The text of <paramref name="key"/> in the current language.</summary>
    public static string Get(string key) => Get(Language, key);

    /// <summary>The text of <paramref name="key"/> in <paramref name="language"/>, else English, else the key.</summary>
    public static string Get(string language, string key)
    {
        if (Tables.TryGetValue(FromTag(language) ?? English, out var table) && table.TryGetValue(key, out var text))
        {
            return text;
        }

        return En.TryGetValue(key, out var english) ? english : key;
    }

    /// <summary>A template with <c>{0}</c>… filled in.</summary>
    public static string Format(string key, params object?[] args) =>
        string.Format(CultureInfo.CurrentCulture, Get(key), args);

    /// <summary>The keys of a language's table (tests).</summary>
    internal static IReadOnlyCollection<string> Keys(string language) =>
        Tables.TryGetValue(language, out var table) ? table.Keys : [];
}

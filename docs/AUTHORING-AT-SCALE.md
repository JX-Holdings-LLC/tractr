# Authoring at scale

Tools for games with a lot of content: dialogue written as text, content
edited as tables and spreadsheets, bulk edits and templates, and
translations of the game's own text. Everything here changes the same
records the Edit Mode forms change, and every change is one undo step.

The pieces live in the F# authoring core (`FarmEngine.Authoring`:
`ConditionText`, `DialogueScript`, `ContentTables`, `BulkEdit`,
`ContentTemplates`, `Translations`), so tools and tests can use them
without the editor.

## Dialogue scripts

An NPC's dialogues and lines, or a set of cutscenes, can be written as a
script: plain text a writer edits in any text editor, read back into the
records the forms edit. The game itself is unchanged.

```text
@npc bram

=== bram.intro
@expression sad
Wheel turns, stones don't. The axle on my grindstone cracked clean through.
* What do you need? -> offer quest-silent-mill
* [if flag mill-fixed] Hear that? -> bram.fixed
* Good luck.

=== bram.lines
[morning] Mornin'. Flour on my boots, flour in my beard.
[weather rain] Rain keeps the river high. Good for the wheel, bad for my knees.
[quest quest-silent-mill active] Ten logs and five good stones.
```

### The format

| Line | Means |
|---|---|
| `@npc ID` | at the top: the NPC the file is for |
| `=== ID` | a dialogue of that NPC (`=== dialogue ID` when the id is `lines`, `cutscene` or `NPC.lines`) |
| `=== NPC.lines` (or `=== lines`) | the NPC's lines of the day |
| `=== cutscene ID` | a cutscene |
| `// …` | a comment |
| `@expression NAME` | the portrait a dialogue shows (one of the NPC's portraits) |
| `@npc ID` inside a dialogue | the dialogue belongs to another NPC |
| `@extra {…}` | keys Tractr does not know, kept as JSON |
| text lines | the dialogue's text; several lines are joined with line breaks |
| `* [GROUPS] TEXT -> TARGETS` | an answer |
| `~ EFFECT`, indented under an answer | what the answer above does |

An answer's groups are `[if CONDITIONS]` and `[once]` (`[once off]` keeps a
paying answer repeatable). Its conditions can check one flag (`flag F`), one
cleared flag (`not flag F`), one item (`item ID`) and this NPC's friendship
(`friendship 250`): those are what an answer stores. Its targets are
`-> DIALOGUE`, `-> offer QUEST`, `-> shop SHOP` and `-> action ACTION`. Its
effects are `~ give item ITEM [N]`, `~ give money N`, `~ take money N`,
`~ set flag FLAG` and `~ extra {…}`.

A line of the day is `[CONDITIONS] [weight N] [@EXPRESSION] TEXT`. Its
conditions can be anything the condition text below can say.

In a cutscene, `SPEAKER: TEXT` is a line (`bram@happy: …` with a portrait,
`player: …`, `: …` for a narrator), `? QUESTION` asks a question with the
`* ANSWER -> LABEL` lines under it, `> jump LABEL if CONDITIONS` jumps,
`@label NAME` names the next step, and `> {…}` is any other step as its
JSON (`> {"type":"wait","seconds":1}`). `@name TEXT` and `@skippable no` go
before the steps.

Text that starts with one of `" * ~ @ = [ ? > : / \ {`, has spaces at its
ends, holds `->`, `#line:` or a line break, or is empty is written in double
quotes, with JSON's escapes (`"Two\nlines"`). Substitutions such as
`{player}` and `{item:wood}` are text like any other.

### Stable ids

Each line, answer and cutscene line can carry an id at its end:
`Mornin'. #line:l-3f2a1c`. Translations find a text by its id, so it keeps
its translation when it moves. When the editor applies a script it gives
every new line and answer an id made from its text.

### Condition text

The same condition text serves lines, answers, cutscene jumps, events and
quests. Conditions are separated by commas:

| Text | Condition |
|---|---|
| `flag NAME`, `flag NAME off` | a flag is set (or not) |
| `item ID [N]`, `space ID [N]` | the player holds N, or has room for N |
| `day A..B`, `day-of-season A..B`, `year A..B` | a range; either end may be left out (`day 3..`) |
| `time 6:00..12:00`, `morning`, `afternoon`, `evening` | the time of day (6:00–11:59, 12:00–17:59, 18:00–23:59) |
| `season S…`, `weather W…`, `weekday D…`, `scene S…` | one of these |
| `quest ID STATUS`, `festival ID` | a quest's status, a festival day |
| `friendship [NPC] N`, `hearts [NPC] A..B`, `relationship [NPC] STATUS`, `birthday NPC` | social (no NPC: this one) |
| `var NAME OP VALUE` | a variable: `=`, `!=`, `<`, `<=`, `>`, `>=`; text in quotes |
| `money N`, `chance 0.25`, `skill S N`, `perk S P C`, `buff ID`, `mine-floor N [MINE]` | the rest |
| `collection ID [N]`, `bundle ID`, `achievement ID`, `shipped ID N`, `building ID N`, `slain ID N` | long-term goals |
| `enter X Y [to X2 Y2]`, `interact X Y [to X2 Y2]` | tile triggers |
| `all(…)`, `any(…)`, `not C`, `not(…)` | groups |
| `{"type": …}` | any condition as its JSON |

A word with spaces or punctuation is written in double quotes.

### Scripts in a project folder

A project saved as a folder keeps each NPC's dialogues and lines in
`scripts/<npc>.dialogue`, and the NPC's file names it (`"dialogueScript"`).
An NPC whose dialogue the format cannot hold exactly stays in JSON. A script
edited outside Tractr is read when the project opens; a mistake stops the
project from loading and names the file and line
(`scripts/bram.dialogue:12: …`). Problems found in a script's dialogue are
reported with the line of the script they are on.

## Content tables

Items, crops, recipes, shop stock, fish tables and node drops can be edited
as tables: a row per entry (per stock entry, fish or drop for the last
three) and a column per field. Crops show their profit per harvest, recipes
their profit per craft and per machine hour; those columns are worked out
and read-only.

- A cell takes text: numbers, `true`/`false`, words separated by commas
  (`spring, summer`) and ingredients as `2 wood, 1 stone`.
- A block copied from a spreadsheet pastes into many rows at once.
- Every cell is checked before anything is written, and a whole edit or
  paste is one undo step.

### CSV

A table saves as CSV (a header of column keys, then a row per entry) and
reads back from one. Rows are matched by their `id` column; a row without an
id is a new one (a new stock entry, fish or drop takes `shop/item`). Before
an import applies, it shows what it would add, change and remove, and every
cell it cannot read with its row and column. Rows the file lacks are only
removed when asked; columns the file lacks keep their values.

## Bulk edits and templates

Select entries of a table by tag, by text, or by picking them, then change
one field across all of them:

| Write | Does |
|---|---|
| `= 50` | sets it |
| `× 1.1` (or `* 1.1`) | multiplies a number |
| `+ 1`, `- 1` | adds to or takes from a number |
| `+ summer`, `- summer` | adds a word to, or takes it out of, a list (seasons, tags) |

The edit shows each change first and applies as one undo step. Every table
entry has tags, labels of your own for selecting entries; the game ignores
them.

A template saves an entry, or a group such as a crop and its harvest item.
Making entries from a template takes a table: a row per new group, with the
fields that differ (`1.id`, `1.name`, `2.value`…). A new id replaces the
template's id everywhere in the group, so the new crop harvests the new
item. An entry given no id gets the template's with `-2`, `-3`…

## Translations

A game's own text (item names and descriptions, NPC names and lines,
dialogues and their answers, quests, cutscene lines, questions and answers,
letters) can be translated into other languages. The project's locale
(**Settings**) is the source language; each other language has a table of
translated texts keyed by a stable key per text field:

| Key | Text |
|---|---|
| `item:ID:name`, `item:ID:description` | an item |
| `npc:ID:name`, `npc:ID:line:LINE` | an NPC and its lines of the day |
| `dialogue:ID:text`, `dialogue:ID:option:OPTION` | a dialogue and its answers |
| `quest:ID:name`, `quest:ID:description` | a quest |
| `cutscene:ID:step:STEP`, `cutscene:ID:step:STEP:choice:CHOICE` | cutscene text |
| `mail:ID:subject`, `mail:ID:text` | a letter |

`LINE`, `OPTION`, `STEP` and `CHOICE` are the text's stable id, or `#3` for
the fourth one without an id. Giving every text an id keeps the
translations made under its number.

A project folder keeps one file per language: `strings/<locale>.json`.

### Translator files

A language exports as CSV (`key`, `context`, `source`, `translation`,
`status`) or as a gettext PO file (`msgctxt` is the key, `msgid` the source,
`#.` the context: who says it and where). Translators fill in the
translation and the file imports back by key; the import says how many texts
it adds and changes, and which keys the game does not have.

### Problems

The Problems panel reports, per language:

- texts not translated yet (the game shows them in its own language);
- stale translations: the source text changed after it was translated (each
  translation keeps a hash of the source it was made from);
- text the game's font cannot draw;
- substitutions (`{player}`, `{item:…}`) a translation drops or adds;
- keys the game no longer has, and a table for the game's own language.

### In the game

Export Game puts every language in the game. The player picks one in
**Settings → Accessibility → Language**, the same picker as the interface's
language; it lists the interface's languages and every language the game is
translated into. Automatic follows the system's language where the game has
it. The game's text changes at once, in a game already running too; text the
translation lacks stays in the source language.

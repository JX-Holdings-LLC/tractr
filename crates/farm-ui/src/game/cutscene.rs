//! The cutscene overlay (M2), from `farm_sim::cutscenes::view`: the screen's fade, then the
//! line box at the bottom (speaker and text, like the dialogue box) with the question's answers
//! when the step asks one, and a Skip button when the cutscene may be skipped. The camera and
//! the emotes are the world renderer's.
//!
//! Keys are the host's (`farm_runtime::input::poll_play_frame`: Interact, Enter or Space moves
//! past a line, 1–9 answer, Escape skips), so the line box takes clicks and taps only and its
//! answers are reached by navigation; nothing here answers a key a second time.

use super::{dialogue, hud, GameAction, GameView};
use crate::input::InputDevice;
use crate::layout::{Align, RectExt};
use crate::settings::BindAction;
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::Button;
use farm_render::{Color, FontId, ImageStore, Rect, TextAlign};
use farm_sim::cutscenes::{self, CutsceneLine, PLAYER};
use farm_sim::Command;

const TEXT_SIZE: f32 = 15.0;
const OPTION_SIZE: f32 = 14.0;

pub(crate) fn draw(ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore, actions: &mut Vec<GameAction>) {
    let Some(cutscene) = cutscenes::view(view.content, view.state) else { return };
    ui.push_layer();
    let screen = ui.screen();
    if let Some(fade) = &cutscene.fade {
        let color = Color::parse(&fade.color).unwrap_or(Color::BLACK);
        let alpha = fade.alpha as f32 / farm_sim::units::MILLI_ONE as f32;
        ui.list_mut().fill_rect(screen, crate::theme::fade(color, alpha.clamp(0.0, 1.0)));
    }
    // A dialogue, shop or minigame an outcome opened is in front and takes the input.
    let open = view.state.dialogue.is_none() && view.state.shop.is_none() && view.state.minigame.is_none();
    if open {
        ui.blocker(screen);
        if let Some(line) = &cutscene.line {
            draw_line(ui, view, images, line, &cutscene.cutscene_id, cutscene.step, actions);
        }
        if cutscene.skippable {
            draw_skip(ui, actions);
        }
    }
    ui.pop_layer();
}

/// The Skip button in the top right corner (pointer only: Escape is the host's).
fn draw_skip(ui: &mut Ui, actions: &mut Vec<GameAction>) {
    let screen = ui.safe_area();
    let label = ui.tr("cutscene.skip");
    let mut button = Button::new(label).size(13.0).focusable(false);
    if ui.device() != InputDevice::Touch {
        button = button.keycap("Esc");
    }
    let width = ui.button_width(&button);
    let height = ui.button_height(13.0);
    let rect = Rect::new(screen.right() - width - 16.0, screen.y + 16.0, width, height);
    if ui.button(WidgetId::new("cutscene-skip"), rect, button) {
        actions.push(GameAction::Command(Command::SkipCutscene));
    }
}

/// The line box: who speaks and what, then the answers or a "Next" hint.
fn draw_line(
    ui: &mut Ui,
    view: &GameView<'_>,
    images: &mut ImageStore,
    line: &CutsceneLine,
    cutscene_id: &str,
    step: u32,
    actions: &mut Vec<GameAction>,
) {
    let colors = ui.theme().colors;
    let screen = ui.screen();
    let speaker = match line.speaker.as_deref() {
        Some(PLAYER) => Some(ui.tr("cutscene.you").to_owned()),
        _ => line.speaker_name.clone(),
    };
    // An NPC with portraits (M4) shows the one for the line's expression.
    let portrait = line
        .speaker
        .as_deref()
        .and_then(|speaker| view.content.npcs.iter().find(|npc| npc.id == speaker))
        .filter(|npc| npc.portraits.as_ref().is_some_and(|portraits| !portraits.is_empty()))
        .and_then(|npc| dialogue::portrait_sprite(view, npc, line.expression.as_deref()));
    let indent = if portrait.is_some() { 56.0 + 16.0 } else { 0.0 };
    let width = 680.0f32.min(screen.width - 32.0);
    let text_width = width - 40.0 - indent;
    let name_height = if speaker.is_some() { ui.line_height(16.5) + 4.0 } else { 0.0 };
    let text_height = ui.paragraph_height(&line.text, TEXT_SIZE, FontId::Regular, text_width);
    let option_heights: Vec<f32> = line
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let key_width = ui.keycap_width(&(index + 1).to_string()) + 10.0;
            let lines = ui.paragraph_height(option, OPTION_SIZE, FontId::Regular, text_width - 28.0 - key_width);
            (lines + 18.0).max(ui.button_height(OPTION_SIZE) + 6.0)
        })
        .collect();
    let footer_height = if line.options.is_empty() {
        ui.line_height(13.0) + 6.0
    } else {
        option_heights.iter().sum::<f32>() + 8.0 * (option_heights.len() - 1) as f32
    };
    let header_height = (name_height + text_height).max(if portrait.is_some() { 56.0 } else { 0.0 });
    let content_height = 20.0 + header_height + 14.0 + footer_height + 20.0;
    let bottom_margin = 24.0 + (screen.height * 0.04).min(40.0);
    let bottom = ui.safe_area().bottom();
    let height = content_height.min((bottom - bottom_margin - 12.0).max(120.0));
    let card = Rect::new((screen.width - width) / 2.0, (bottom - bottom_margin - height).max(12.0), width, height);
    ui.panel(card, colors.panel, colors.panel_border, 12.0, true);
    if line.options.is_empty() {
        // Clicking or tapping the box moves on (keys are the host's).
        let state =
            ui.interact(WidgetId::new("cutscene-line").with(cutscene_id).with(step), card, HitKind::Button, false);
        if state.clicked {
            actions.push(GameAction::Command(Command::AdvanceCutscene));
        }
    } else {
        ui.blocker(card);
    }

    if let Some(sprite) = &portrait {
        let frame = Rect::new(card.x + 20.0, card.y + 20.0, 56.0, 56.0);
        ui.list_mut().fill_circle(frame.x + 28.0, frame.y + 28.0, 28.0, colors.secondary);
        ui.list_mut().fill_circle(frame.x + 28.0, frame.y + 28.0, 25.0, colors.row);
        super::draw_sprite(ui, images, sprite, frame.centered(48.0, 48.0), view.art.pixel_art);
    }
    let left = card.x + 20.0 + indent;
    let mut y = card.y + 20.0;
    if let Some(speaker) = &speaker {
        ui.label(
            Rect::new(left, y, text_width, ui.line_height(16.5)),
            speaker,
            16.5,
            FontId::Bold,
            colors.text,
            Align::Start,
        );
        y += name_height;
    }
    ui.paragraph(left, y, text_width, &line.text, TEXT_SIZE, FontId::Regular, colors.text, TextAlign::Left);
    y = card.y + 20.0 + header_height + 14.0;

    if line.options.is_empty() {
        // "Next" with the interact key, right-aligned.
        let lang = ui.lang();
        let label = ui.tr("cutscene.next");
        let row = Rect::new(left, y, text_width, ui.line_height(13.0) + 6.0);
        let label_width = ui.measure(label, 13.0, FontId::Bold);
        let key =
            (ui.device() != InputDevice::Touch).then(|| hud::prompt(view, ui.device(), BindAction::Interact, lang));
        let key_width = key.as_deref().map_or(0.0, |key| ui.keycap_width(key) + 8.0);
        let mut x = row.right() - label_width - key_width;
        if let Some(key) = key.as_deref() {
            x += ui.keycap(x, row.y + row.height / 2.0, key, true) + 8.0;
        }
        ui.label(Rect::new(x, row.y, label_width, row.height), label, 13.0, FontId::Bold, colors.muted, Align::Start);
        return;
    }
    // The answers, drawn like the dialogue's options (text wraps); the first takes the focus.
    for (index, (option, height)) in line.options.iter().zip(&option_heights).enumerate() {
        let rect = Rect::new(left, y, text_width, *height);
        let id = WidgetId::new("cutscene-option").with(cutscene_id).with(step).with(index);
        let state = if index == 0 {
            ui.interact_default(id, rect, HitKind::Button)
        } else {
            ui.interact(id, rect, HitKind::Button, true)
        };
        let hovered = state.hovered || state.focus_visible;
        let fill = if state.active {
            colors.button_pressed
        } else if hovered {
            colors.button_hover
        } else {
            colors.button
        };
        let radius = ui.theme().small_radius;
        ui.list_mut().fill_round_rect(rect, radius, fill);
        ui.list_mut().stroke_round_rect(
            rect.inset(0.5),
            radius,
            if hovered { colors.accent } else { colors.button_border },
            1.0,
        );
        if state.focus_visible {
            ui.focus_ring(rect, radius);
        }
        let text_top = rect.y + 9.0;
        let key_width =
            ui.keycap(rect.x + 14.0, text_top + ui.line_height(OPTION_SIZE) / 2.0, &(index + 1).to_string(), true);
        let text_left = rect.x + 14.0 + key_width + 10.0;
        ui.paragraph(
            text_left,
            text_top,
            rect.right() - 14.0 - text_left,
            option,
            OPTION_SIZE,
            FontId::Regular,
            colors.text,
            TextAlign::Left,
        );
        if state.clicked {
            actions.push(GameAction::Command(Command::ChooseCutsceneOption { index: index as i32 }));
        }
        y += height + 8.0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Fixture;
    use super::super::GameAction;
    use crate::input::NavAction;
    use crate::ui::WidgetId;
    use farm_render::{Color, DrawCmd};
    use farm_sim::schema::{Cutscene, CutsceneFade, CutsceneState};
    use farm_sim::Command;
    use serde_json::json;

    fn command(actions: &[GameAction]) -> Option<&Command> {
        actions.iter().find_map(|action| match action {
            GameAction::Command(command) => Some(command),
            _ => None,
        })
    }

    /// The starter farm playing a cutscene of these steps (JSON) at step 0.
    fn playing(steps: serde_json::Value, skippable: bool) -> Fixture {
        let mut fixture = Fixture::starter();
        let cutscene: Cutscene =
            serde_json::from_value(json!({"id": "intro", "name": "Intro", "steps": steps, "skippable": skippable}))
                .unwrap();
        fixture.ctx.content.cutscenes.push(cutscene);
        fixture.state.cutscene = Some(CutsceneState { cutscene_id: "intro".into(), ..CutsceneState::default() });
        fixture.idle();
        fixture
    }

    #[test]
    fn a_line_shows_its_speaker_and_moves_on_when_clicked() {
        let mut fixture =
            playing(json!([{"type": "say", "speaker": "npc-merchant", "text": "Welcome to town."}]), true);
        let texts = fixture.texts();
        assert!(texts.iter().any(|text| text == "Merchant Mia"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "Welcome to town."), "{texts:?}");
        assert!(texts.iter().any(|text| text == "Next"), "{texts:?}");
        let line = WidgetId::new("cutscene-line").with("intro").with(0u32);
        assert_eq!(command(&fixture.click(line)), Some(&Command::AdvanceCutscene));
        assert_eq!(command(&fixture.click(WidgetId::new("cutscene-skip"))), Some(&Command::SkipCutscene));
        // Keys are the host's: accepting or pressing a digit here sends nothing.
        assert_eq!(command(&fixture.nav(&[NavAction::Accept])), None);
        assert_eq!(command(&fixture.keys(&["1"])), None);
    }

    #[test]
    fn the_player_speaks_as_you_and_unskippable_cutscenes_have_no_skip_button() {
        let mut fixture = playing(json!([{"type": "say", "speaker": "player", "text": "Hm."}]), false);
        assert!(fixture.texts().iter().any(|text| text == "You"));
        assert!(fixture.ui.last_rect(WidgetId::new("cutscene-skip")).is_none());
        fixture.ui.set_lang(crate::i18n::Lang::Es);
        fixture.idle();
        assert!(fixture.texts().iter().any(|text| text == "T\u{fa}"));
    }

    #[test]
    fn answers_are_picked_by_click_and_navigation() {
        let steps = json!([{"type": "choice", "text": "Stay a while?", "options": [{"text": "Yes"}, {"text": "No"}]}]);
        let mut fixture = playing(steps.clone(), true);
        let texts = fixture.texts();
        assert!(texts.iter().any(|text| text == "Stay a while?") && texts.iter().any(|text| text == "No"));
        let second = WidgetId::new("cutscene-option").with("intro").with(0u32).with(1usize);
        assert_eq!(command(&fixture.click(second)), Some(&Command::ChooseCutsceneOption { index: 1 }));
        // The first answer has the focus; Down moves it to the second.
        let mut fixture = playing(steps, true);
        fixture.nav(&[NavAction::Down]);
        assert_eq!(command(&fixture.nav(&[NavAction::Accept])), Some(&Command::ChooseCutsceneOption { index: 1 }));
        assert_eq!(command(&fixture.keys(&["1"])), None, "digits are the host's");
    }

    #[test]
    fn a_fade_covers_the_screen_in_its_colour() {
        let mut fixture = playing(json!([{"type": "wait"}]), true);
        let running = fixture.state.cutscene.as_mut().unwrap();
        running.fade = Some(CutsceneFade { color: "#ff0000".into(), from: 0, to: 1000, duration: 20, elapsed: 10 });
        fixture.idle();
        let cover = fixture.last.commands.iter().find_map(|command| match command {
            DrawCmd::FillRect { rect, color, .. } if rect.width >= 1280.0 && rect.height >= 800.0 => Some(*color),
            _ => None,
        });
        let cover = cover.expect("a full-screen fill");
        assert_eq!((cover.r, cover.g, cover.b), (255, 0, 0));
        assert!((120..=135).contains(&cover.a), "{cover:?}");
        assert_ne!(cover, Color::TRANSPARENT);
    }
}

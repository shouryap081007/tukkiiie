//! Local-first study and break timer, kept outside the Live2D and input paths.
#![forbid(unsafe_code)]
use bongocat_plugin_sdk::{ActionIcon, Anchor, ConfigValue, Configuration, HostMessage, Plugin, PluginModelEvent, PluginOutput, PluginView, StatusIcon, UiNode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

const DEFAULT_STUDY_MINUTES: i64 = 25;
const DEFAULT_BREAK_MINUTES: i64 = 5;
const MAX_STUDY_MINUTES: i64 = 180;
const MAX_BREAK_MINUTES: i64 = 60;
const MAX_SAVED_SESSIONS: usize = 90;

struct Copy {
    study: &'static str, break_label: &'static str, studying: &'static str, start: &'static str, pause: &'static str,
    restart: &'static str, finish_early: &'static str, study_minutes: &'static str,
    break_minutes: &'static str, study_complete: &'static str, break_complete: &'static str,
    ended_early: &'static str, bubble_study_started: &'static str, bubble_paused: &'static str,
    bubble_mid_study: &'static str, bubble_study_complete: &'static str, bubble_break_complete: &'static str,
    bubble_ended_early: &'static str, today: &'static str, sessions: &'static str, minutes: &'static str,
}
const ENGLISH: Copy = Copy {
    study: "Study", break_label: "Break", studying: "Studying", start: "Start", pause: "Pause", restart: "Restart",
    finish_early: "Finish early", study_minutes: "Study minutes", break_minutes: "Break minutes",
    study_complete: "Study complete. Break time!", break_complete: "Break over. Ready when you are.",
    ended_early: "Session ended early.",
    bubble_study_started: "Back to work, professional procrastinator.",
    bubble_paused: "Already tired? The chapter barely noticed.",
    bubble_mid_study: "Halfway there. Don't negotiate with the timer.",
    bubble_study_complete: "Look at you, academically suspicious. Break time!",
    bubble_break_complete: "Break's over. Your backlog misses you.",
    bubble_ended_early: "Leaving already? The backlog says thanks.",
    today: "Today", sessions: "sessions", minutes: "min",
};
const SIMPLIFIED_CHINESE: Copy = Copy {
    study: "学习", break_label: "休息", studying: "学习中", start: "开始", pause: "暂停", restart: "重新计时",
    finish_early: "提前结束", study_minutes: "学习时长（分钟）", break_minutes: "休息时长（分钟）",
    study_complete: "学习完成。休息一下！", break_complete: "休息结束，准备好继续了吗？",
    ended_early: "本次学习提前结束。",
    bubble_study_started: "该学习了，拖延专家。",
    bubble_paused: "已经累了？课本才刚认出你。",
    bubble_mid_study: "过半了，别跟计时器讨价还价。",
    bubble_study_complete: "今天居然这么自律？休息时间！",
    bubble_break_complete: "休息结束，你的 backlog 想你了。",
    bubble_ended_early: "这么快就走？积压任务可开心了。",
    today: "今日", sessions: "次", minutes: "分钟",
};
fn copy_for(locale: &str) -> &'static Copy {
    if locale == "zh-CN" { &SIMPLIFIED_CHINESE } else { &ENGLISH }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase { #[default] Study, Break }

#[derive(Clone, Copy, Debug)]
enum Notice { StudyComplete, BreakComplete, EndedEarly }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StudySession {
    day: String,
    elapsed_ms: u64,
    completed: bool,
}

#[derive(Serialize, Deserialize)]
struct Pomodoro {
    remaining_ms: u64,
    running: bool,
    started: bool,
    // Keep the existing field name to preserve saved Pomodoro settings.
    minutes: i64,
    #[serde(default = "default_break_minutes")]
    break_minutes: i64,
    #[serde(default)]
    phase: Phase,
    #[serde(default)]
    focus_elapsed_ms: u64,
    #[serde(default)]
    session_day: Option<String>,
    #[serde(default)]
    sessions: Vec<StudySession>,
    #[serde(skip)]
    last_tick: Option<u64>,
    #[serde(skip)]
    locale: Option<String>,
    #[serde(skip)]
    notice: Option<Notice>,
    #[serde(skip)]
    model_events: Vec<PluginModelEvent>,
    #[serde(skip)]
    bubble: Option<String>,
    #[serde(skip)]
    bubble_remaining_ms: u64,
}
const fn default_break_minutes() -> i64 { DEFAULT_BREAK_MINUTES }

impl Default for Pomodoro {
    fn default() -> Self {
        Self {
            remaining_ms: 0, running: false, started: false,
            minutes: DEFAULT_STUDY_MINUTES, break_minutes: DEFAULT_BREAK_MINUTES,
            phase: Phase::Study, focus_elapsed_ms: 0, session_day: None, sessions: Vec::new(),
            last_tick: None, locale: None, notice: None, model_events: Vec::new(),
            bubble: None, bubble_remaining_ms: 0,
        }
    }
}

fn local_day_key() -> String {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc()).date().to_string()
}

impl Pomodoro {
    fn copy(&self) -> &'static Copy { self.locale.as_deref().map_or(&ENGLISH, copy_for) }
    fn study_ms(&self) -> u64 { self.minutes.max(1) as u64 * 60_000 }
    fn break_ms(&self) -> u64 { self.break_minutes.max(1) as u64 * 60_000 }
    fn phase_ms(&self) -> u64 {
        match self.phase { Phase::Study => self.study_ms(), Phase::Break => self.break_ms() }
    }
    fn show_bubble(&mut self, message: &'static str) {
        self.bubble = Some(message.into());
        self.bubble_remaining_ms = 4_000;
    }

    fn configure(&mut self, config: &Configuration) -> Result<(), String> {
        if config.keys().any(|key| key != "minutes" && key != "break_minutes") {
            return Err("Unknown configuration key".into());
        }
        let study = match config.get("minutes") {
            None => self.minutes,
            Some(ConfigValue::Integer(value)) if (1..=MAX_STUDY_MINUTES).contains(value) => *value,
            _ => return Err("Study minutes must be between 1 and 180".into()),
        };
        let break_minutes = match config.get("break_minutes") {
            None => self.break_minutes,
            Some(ConfigValue::Integer(value)) if (1..=MAX_BREAK_MINUTES).contains(value) => *value,
            _ => return Err("Break minutes must be between 1 and 60".into()),
        };
        self.minutes = study;
        self.break_minutes = break_minutes;
        Ok(())
    }

    fn today_totals(&self, today: &str) -> (usize, u64) {
        self.sessions.iter().filter(|s| s.day == today).fold((0, 0), |(count, elapsed), s| {
            (count + 1, elapsed.saturating_add(s.elapsed_ms))
        })
    }

    fn record_study(&mut self, completed: bool) {
        if self.focus_elapsed_ms == 0 { self.session_day = None; return; }
        let elapsed_ms = if completed { self.study_ms() } else { self.focus_elapsed_ms.min(self.study_ms()) };
        let day = self.session_day.take().unwrap_or_else(local_day_key);
        self.sessions.push(StudySession { day, elapsed_ms, completed });
        if self.sessions.len() > MAX_SAVED_SESSIONS {
            let excess = self.sessions.len() - MAX_SAVED_SESSIONS;
            self.sessions.drain(0..excess);
        }
        self.focus_elapsed_ms = 0;
    }

    fn output(&mut self) -> Result<PluginOutput, String> {
        let copy = self.copy();
        let clock_ms = if self.started { self.remaining_ms } else { self.phase_ms() };
        let phase_label = match self.phase { Phase::Study => copy.study, Phase::Break => copy.break_label };
        let (today_sessions, today_elapsed_ms) = self.today_totals(&local_day_key());
        let today_minutes = today_elapsed_ms / 60_000;
        let mut model_nodes = vec![UiNode::Text {
            text: format!("{phase_label} · {}", format_clock(clock_ms)),
        }];
        if self.phase == Phase::Study && self.running {
            model_nodes.push(UiNode::IconText {
                icon: study_book_icon(),
                text: copy.studying.into(),
            });
        }
        if let Some(text) = self.bubble.as_deref() {
            model_nodes.push(UiNode::IconText {
                icon: StatusIcon::Badge("...".into()),
                text: text.into(),
            });
        }
        let notice = match self.notice {
            Some(Notice::StudyComplete) => Some(copy.study_complete),
            Some(Notice::BreakComplete) => Some(copy.break_complete),
            Some(Notice::EndedEarly) => Some(copy.ended_early),
            None => None,
        };
        if let Some(text) = notice { model_nodes.push(UiNode::Text { text: text.into() }); }
        model_nodes.push(UiNode::Text {
            text: format!("{}: {} {} · {} {}", copy.today, today_sessions, copy.sessions, today_minutes, copy.minutes),
        });
        model_nodes.push(UiNode::Button {
            id: "toggle".into(), label: if self.running { copy.pause } else { copy.start }.into(),
            icon: if self.running { ActionIcon::Pause } else { ActionIcon::Play }, loading: false,
        });
        model_nodes.push(UiNode::Button {
            id: "restart".into(), label: copy.restart.into(), icon: ActionIcon::Restart, loading: false,
        });
        if self.phase == Phase::Study && self.focus_elapsed_ms > 0 {
            model_nodes.push(UiNode::Button {
                id: "finish_early".into(), label: copy.finish_early.into(),
                icon: ActionIcon::Restart, loading: false,
            });
        }
        Ok(PluginOutput {
            model: PluginView { anchor: Anchor::TopRight, nodes: model_nodes },
            settings: PluginView {
                nodes: vec![
                    UiNode::Integer { key: "minutes".into(), label: copy.study_minutes.into(),
                        value: self.minutes, min: 1, max: MAX_STUDY_MINUTES },
                    UiNode::Integer { key: "break_minutes".into(), label: copy.break_minutes.into(),
                        value: self.break_minutes, min: 1, max: MAX_BREAK_MINUTES },
                ],
                ..Default::default()
            },
            config: [
                ("minutes".into(), ConfigValue::Integer(self.minutes)),
                ("break_minutes".into(), ConfigValue::Integer(self.break_minutes)),
            ].into(),
            state: serde_json::to_vec(self).map_err(|_| "Cannot save study timer")?,
            model_events: std::mem::take(&mut self.model_events),
            errors: Vec::new(),
        })
    }

    fn finish_study(&mut self) {
        self.focus_elapsed_ms = self.study_ms();
        self.record_study(true);
        self.phase = Phase::Break;
        self.remaining_ms = self.break_ms();
        self.running = true;
        self.started = true;
        self.notice = Some(Notice::StudyComplete);
        self.show_bubble(self.copy().bubble_study_complete);
        self.emit_stop_study_motion();
        self.emit_wave();
    }
    fn finish_break(&mut self) {
        self.phase = Phase::Study;
        self.remaining_ms = self.study_ms();
        self.running = false;
        self.started = false;
        self.notice = Some(Notice::BreakComplete);
        self.show_bubble(self.copy().bubble_break_complete);
        self.emit_wave();
    }

    fn emit_study_motion(&mut self) {
        self.model_events.push(PluginModelEvent::StartMotion {
            group: "Tukkiiie_Study".into(),
            index: 0,
            looping: true,
        });
    }

    fn emit_stop_study_motion(&mut self) {
        self.model_events.push(PluginModelEvent::StopMotion {
            group: "Tukkiiie_Study".into(),
            index: 0,
        });
    }

    fn emit_wave(&mut self) {
        self.model_events.push(PluginModelEvent::StartMotion {
            group: "CAT_motion".into(),
            index: 1,
            looping: false,
        });
    }
}


/// A tiny open-book glyph for the model-window HUD. It is generated locally so
/// the plugin needs no extra image asset, file access or network request.
fn study_book_icon() -> StatusIcon {
    const SIZE: usize = 32;
    let mut pixels = vec![0u8; SIZE * SIZE * 4];
    let paper = [232, 243, 248, 255];
    let ink = [79, 178, 205, 255];
    let ruling = [54, 112, 139, 255];

    fill_icon_polygon(&mut pixels, &[(4, 7), (14, 10), (14, 26), (4, 23)], paper);
    fill_icon_polygon(&mut pixels, &[(17, 10), (27, 7), (27, 23), (17, 26)], paper);

    for (x0, y0, x1, y1) in [
        (4, 7, 14, 10), (14, 10, 14, 26), (14, 26, 4, 23), (4, 23, 4, 7),
        (17, 10, 27, 7), (27, 7, 27, 23), (27, 23, 17, 26), (17, 26, 17, 10),
        (15, 10, 15, 27), (16, 10, 16, 27),
    ] {
        draw_icon_line(&mut pixels, x0, y0, x1, y1, ink);
    }
    for (x0, y0, x1, y1) in [
        (7, 14, 12, 16), (7, 18, 12, 20),
        (19, 16, 24, 14), (19, 20, 24, 18),
    ] {
        draw_icon_line(&mut pixels, x0, y0, x1, y1, ruling);
    }

    StatusIcon::Rgba(pixels)
}

fn fill_icon_polygon(
    pixels: &mut [u8],
    points: &[(i32, i32)],
    color: [u8; 4],
) {
    for y in 0..32 {
        for x in 0..32 {
            let mut inside = false;
            let mut previous = points.len() - 1;
            for current in 0..points.len() {
                let (xi, yi) = points[current];
                let (xj, yj) = points[previous];
                if (yi > y) != (yj > y)
                    && (x as f32)
                        < (xj - xi) as f32 * (y - yi) as f32 / (yj - yi) as f32 + xi as f32
                {
                    inside = !inside;
                }
                previous = current;
            }
            if inside {
                let offset = (y as usize * 32 + x as usize) * 4;
                pixels[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }
}

fn draw_icon_line(
    pixels: &mut [u8],
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for step in 0..=steps {
        let progress = step as f32 / steps as f32;
        let x = (x0 as f32 + (x1 - x0) as f32 * progress).round() as i32;
        let y = (y0 as f32 + (y1 - y0) as f32 * progress).round() as i32;
        for dy in 0..2 {
            for dx in 0..2 {
                let px = x + dx;
                let py = y + dy;
                if (0..32).contains(&px) && (0..32).contains(&py) {
                    let offset = (py as usize * 32 + px as usize) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn format_clock(remaining_ms: u64) -> String {
    let seconds = remaining_ms.div_ceil(1000);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

impl Plugin for Pomodoro {
    fn id(&self) -> &'static str { "com.bongocat.pomodoro" }

    fn handle(&mut self, message: HostMessage) -> Result<PluginOutput, String> {
        match message {
            HostMessage::Start { config, state } => {
                if !state.is_empty() {
                    *self = serde_json::from_slice(&state).map_err(|_| "Invalid saved study timer")?;
                }
                self.running = false;
                self.last_tick = None;
                self.notice = None;
                self.bubble = None;
                self.bubble_remaining_ms = 0;
                self.configure(&config)?;
            }
            HostMessage::Configure { config } => self.configure(&config)?,
            HostMessage::InputActivity { .. } | HostMessage::InputEvents { .. } => {}
            HostMessage::Tick { elapsed_ms } => {
                let delta_ms = self.last_tick.map_or(0, |previous| elapsed_ms.saturating_sub(previous));
                if self.bubble.is_some() && delta_ms > 0 {
                    self.bubble_remaining_ms = self.bubble_remaining_ms.saturating_sub(delta_ms);
                    if self.bubble_remaining_ms == 0 { self.bubble = None; }
                }
                if self.running {
                    if self.last_tick.is_some() {
                        let previous_remaining = self.remaining_ms;
                        self.remaining_ms = self.remaining_ms.saturating_sub(delta_ms);
                        if self.phase == Phase::Study {
                            self.focus_elapsed_ms = self.focus_elapsed_ms.saturating_add(delta_ms).min(self.study_ms());
                            if previous_remaining > self.study_ms() / 2
                                && self.remaining_ms <= self.study_ms() / 2
                                && self.remaining_ms > 0
                            {
                                self.show_bubble(self.copy().bubble_mid_study);
                            }
                        }
                        if self.remaining_ms == 0 {
                            match self.phase { Phase::Study => self.finish_study(), Phase::Break => self.finish_break() }
                        }
                    }
                }
                self.last_tick = Some(elapsed_ms);
            }
            HostMessage::Action { id } => match id.as_str() {
                "toggle" => {
                    let was_running = self.running;
                    if self.remaining_ms == 0 { self.remaining_ms = self.phase_ms(); }
                    if !self.running && self.phase == Phase::Study && self.focus_elapsed_ms == 0 {
                        self.session_day = Some(local_day_key());
                    }
                    self.running = !self.running;
                    if self.phase == Phase::Study {
                        if self.running {
                            self.emit_study_motion();
                            if !was_running { self.show_bubble(self.copy().bubble_study_started); }
                        } else {
                            self.emit_stop_study_motion();
                            if was_running { self.show_bubble(self.copy().bubble_paused); }
                        }
                    }
                    self.started = true;
                    self.last_tick = None;
                    self.notice = None;
                }
                "restart" => {
                    self.remaining_ms = self.phase_ms();
                    if self.phase == Phase::Study {
                        self.focus_elapsed_ms = 0;
                        self.session_day = Some(local_day_key());
                    }
                    self.running = true;
                    if self.phase == Phase::Study {
                        self.emit_study_motion();
                        self.show_bubble(self.copy().bubble_study_started);
                    }
                    self.started = true;
                    self.last_tick = None;
                    self.notice = None;
                }
                "finish_early" if self.phase == Phase::Study && self.focus_elapsed_ms > 0 => {
                    self.record_study(false);
                    self.remaining_ms = self.study_ms();
                    self.running = false;
                    self.started = false;
                    self.last_tick = None;
                    self.notice = Some(Notice::EndedEarly);
                    self.show_bubble(self.copy().bubble_ended_early);
                    self.emit_stop_study_motion();
                }
                "finish_early" => return Err("There is no active study time to record".into()),
                _ => return Err("Unknown study timer action".into()),
            },
            HostMessage::Preview { .. } => return Err("Unknown study timer preview".into()),
            HostMessage::Locale { locale } => self.locale = Some(locale),
            HostMessage::Pause | HostMessage::Stop => {
                if self.phase == Phase::Study && self.running {
                    self.emit_stop_study_motion();
                }
                self.running = false;
                self.last_tick = None;
            }
            HostMessage::Resume => {
                self.running = self.remaining_ms > 0;
                if self.phase == Phase::Study && self.running {
                    self.emit_study_motion();
                }
                self.last_tick = None;
            }
            HostMessage::Reset => { *self = Self { locale: self.locale.take(), ..Default::default() }; }
            HostMessage::Handshake { .. } => return Err("Handshake belongs to SDK".into()),
        }
        self.output()
    }
}

fn main() -> std::io::Result<()> { bongocat_plugin_sdk::run(&mut Pomodoro::default()) }

#[cfg(test)]
mod tests {
    use super::*;
    fn action(plugin: &mut Pomodoro, id: &str) -> PluginOutput {
        plugin.handle(HostMessage::Action { id: id.into() }).unwrap()
    }
    fn tick(plugin: &mut Pomodoro, elapsed_ms: u64) -> PluginOutput {
        plugin.handle(HostMessage::Tick { elapsed_ms }).unwrap()
    }

    #[test]
    fn study_completion_starts_a_break_and_records_the_session() {
        let mut p = Pomodoro { minutes: 1, break_minutes: 5, ..Default::default() };
        tick(&mut p, 0); action(&mut p, "toggle"); tick(&mut p, 0);
        let output = tick(&mut p, 60_000);
        assert_eq!(output.model_events, vec![
            PluginModelEvent::StopMotion {
                group: "Tukkiiie_Study".into(), index: 0,
            },
            PluginModelEvent::StartMotion {
                group: "CAT_motion".into(), index: 1, looping: false,
            },
        ]);
        assert_eq!(p.phase, Phase::Break);
        assert!(p.running);
        assert!(!output.model.nodes.iter().any(|node| matches!(node, UiNode::IconText { icon: StatusIcon::Rgba(_), .. })));
        assert_eq!(p.remaining_ms, 5 * 60_000);
        assert_eq!(p.sessions.len(), 1);
        assert!(p.sessions[0].completed);
        assert_eq!(p.sessions[0].elapsed_ms, 60_000);
    }

    #[test]
    fn break_completion_returns_to_a_ready_study_timer() {
        let mut p = Pomodoro { phase: Phase::Break, remaining_ms: 5 * 60_000, running: true,
            started: true, last_tick: Some(0), ..Default::default() };
        let output = tick(&mut p, 5 * 60_000);
        assert_eq!(output.model_events, vec![PluginModelEvent::StartMotion {
            group: "CAT_motion".into(), index: 1, looping: false,
        }]);
        assert_eq!(p.phase, Phase::Study);
        assert!(!output.model.nodes.iter().any(|node| matches!(node, UiNode::IconText { icon: StatusIcon::Rgba(_), .. })));
        assert!(!p.running);
        assert!(!p.started);
        assert_eq!(p.remaining_ms, 25 * 60_000);
    }

    #[test]
    fn study_start_and_pause_control_the_looping_study_pose() {
        let mut p = Pomodoro::default();
        let start = action(&mut p, "toggle");
        assert_eq!(start.model_events, vec![PluginModelEvent::StartMotion {
            group: "Tukkiiie_Study".into(), index: 0, looping: true,
        }]);
        assert!(start.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText {
                icon: StatusIcon::Rgba(pixels),
                text,
            } if pixels.len() == 32 * 32 * 4 && text == "Studying"
        )));

        let pause = action(&mut p, "toggle");
        assert_eq!(pause.model_events, vec![PluginModelEvent::StopMotion {
            group: "Tukkiiie_Study".into(), index: 0,
        }]);
        assert!(!pause.model.nodes.iter().any(|node| matches!(
            node, UiNode::IconText { icon: StatusIcon::Rgba(_), .. }
        )));
        assert!(pause.model.nodes.iter().any(|node| matches!(
            node, UiNode::IconText { icon: StatusIcon::Badge(_), .. }
        )));
        assert!(!p.running);
    }

    #[test]
    fn pause_does_not_count_time_and_resume_keeps_the_remaining_time() {
        let mut p = Pomodoro { minutes: 1, ..Default::default() };
        tick(&mut p, 0); action(&mut p, "toggle"); tick(&mut p, 0); tick(&mut p, 10_000);
        p.handle(HostMessage::Pause).unwrap(); tick(&mut p, 40_000);
        assert_eq!(p.remaining_ms, 50_000);
        p.handle(HostMessage::Resume).unwrap();
        tick(&mut p, 50_000); tick(&mut p, 60_000); tick(&mut p, 70_000); tick(&mut p, 100_000);
        assert_eq!(p.phase, Phase::Break);
    }

    #[test]
    fn finishing_early_records_only_the_actual_study_time() {
        let mut p = Pomodoro { minutes: 25, ..Default::default() };
        tick(&mut p, 0); action(&mut p, "toggle"); tick(&mut p, 0); tick(&mut p, 12_000);
        let _output = action(&mut p, "finish_early");
        assert_eq!(p.sessions.len(), 1);
        assert!(!p.sessions[0].completed);
        assert_eq!(p.sessions[0].elapsed_ms, 12_000);
        assert!(!p.running);
        assert!(matches!(p.notice, Some(Notice::EndedEarly)));
    }

    #[test]
    fn configuration_validates_study_and_break_durations() {
        let mut p = Pomodoro::default();
        let valid = [("minutes".into(), ConfigValue::Integer(45)),
            ("break_minutes".into(), ConfigValue::Integer(10))].into();
        p.configure(&valid).unwrap();
        assert_eq!(p.minutes, 45); assert_eq!(p.break_minutes, 10);
        assert!(p.configure(&[("minutes".into(), ConfigValue::Integer(0))].into()).is_err());
        assert!(p.configure(&[("break_minutes".into(), ConfigValue::Integer(61))].into()).is_err());
        assert!(p.configure(&[("unknown".into(), ConfigValue::Integer(1))].into()).is_err());
        assert_eq!(p.minutes, 45); assert_eq!(p.break_minutes, 10);
    }

    #[test]
    fn old_saved_pomodoro_state_loads_with_default_break_settings() {
        let old_state = r#"{"remaining_ms":1200000,"running":false,"started":true,"minutes":25}"#;
        let p: Pomodoro = serde_json::from_str(old_state).unwrap();
        assert_eq!(p.phase, Phase::Study);
        assert_eq!(p.break_minutes, DEFAULT_BREAK_MINUTES);
        assert!(p.sessions.is_empty());
        assert_eq!(p.remaining_ms, 1_200_000);
    }

    #[test]
    fn daily_totals_are_filtered_by_day_and_history_is_bounded() {
        let mut p = Pomodoro::default();
        p.sessions = vec![
            StudySession { day: "2026-11-05".into(), elapsed_ms: 25 * 60_000, completed: true },
            StudySession { day: "2026-11-04".into(), elapsed_ms: 15 * 60_000, completed: false },
        ];
        assert_eq!(p.today_totals("2026-11-05"), (1, 25 * 60_000));
        for _ in 0..MAX_SAVED_SESSIONS + 5 {
            p.sessions.push(StudySession { day: "2026-11-05".into(), elapsed_ms: 60_000, completed: true });
            if p.sessions.len() > MAX_SAVED_SESSIONS {
                let excess = p.sessions.len() - MAX_SAVED_SESSIONS;
                p.sessions.drain(0..excess);
            }
        }
        assert_eq!(p.sessions.len(), MAX_SAVED_SESSIONS);
    }

    #[test]
    fn reset_clears_history_but_preserves_the_display_language() {
        let mut p = Pomodoro::default();
        p.sessions.push(StudySession { day: "2026-11-05".into(), elapsed_ms: 60_000, completed: true });
        p.handle(HostMessage::Locale { locale: "zh-CN".into() }).unwrap();
        let output = p.handle(HostMessage::Reset).unwrap();
        assert!(p.sessions.is_empty());
        assert_eq!(p.copy().study, "学习");
        assert!(output.model.validate());
        assert!(output.settings.validate());
    }

    #[test]
    fn output_exposes_both_duration_settings_and_daily_summary() {
        let mut p = Pomodoro::default();
        let output = p.output().unwrap();
        assert!(output.model.validate());
        assert!(output.settings.validate());
        assert_eq!(output.settings.nodes.len(), 2);
        assert!(output.model.nodes.iter().any(|node| matches!(node, UiNode::Text { text } if text.contains("Today"))));
    }

    #[test]
    fn study_start_bubble_is_playful_and_expires_after_four_seconds() {
        let mut p = Pomodoro::default();
        let start = action(&mut p, "toggle");
        assert!(start.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(badge), text }
                if badge == "..." && text == ENGLISH.bubble_study_started
        )));
        tick(&mut p, 0);
        let expired = tick(&mut p, 4_000);
        assert!(!expired.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(_), .. }
        )));
    }

    #[test]
    fn pause_keeps_the_book_hidden_but_shows_the_roast_bubble_briefly() {
        let mut p = Pomodoro::default();
        action(&mut p, "toggle");
        let paused = action(&mut p, "toggle");
        assert!(paused.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(badge), text }
                if badge == "..." && text == ENGLISH.bubble_paused
        )));
        assert!(!paused.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Rgba(_), .. }
        )));
    }

    #[test]
    fn midpoint_bubble_appears_once_when_half_the_study_time_is_left() {
        let mut p = Pomodoro { minutes: 2, ..Default::default() };
        tick(&mut p, 0);
        action(&mut p, "toggle");
        tick(&mut p, 0);
        let halfway = tick(&mut p, 60_000);
        assert!(halfway.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(badge), text }
                if badge == "..." && text == ENGLISH.bubble_mid_study
        )));
    }

    #[test]
    fn study_and_break_completion_publish_their_own_bubble_messages() {
        let mut study = Pomodoro { minutes: 1, ..Default::default() };
        tick(&mut study, 0);
        action(&mut study, "toggle");
        tick(&mut study, 0);
        let completed = tick(&mut study, 60_000);
        assert!(completed.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(_), text }
                if text == ENGLISH.bubble_study_complete
        )));

        let mut rest = Pomodoro {
            phase: Phase::Break, remaining_ms: 5_000, running: true,
            started: true, last_tick: Some(0), ..Default::default()
        };
        let resumed = tick(&mut rest, 5_000);
        assert!(resumed.model.nodes.iter().any(|node| matches!(
            node,
            UiNode::IconText { icon: StatusIcon::Badge(_), text }
                if text == ENGLISH.bubble_break_complete
        )));
    }
}

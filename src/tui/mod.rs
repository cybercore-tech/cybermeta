//! Interactive EXIF workstation (ratatui + CYBERGRID theme).

use crate::meta::{list_presets, MetaDoc, Preset, TagView};
use crate::theme::Theme;
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Browse,
    Filter,
    Edit,
    PathInput,
    ConfirmStripAll,
    PresetPicker,
    Message,
}

pub fn run(initial: Option<PathBuf>) -> Result<()> {
    let mut terminal = setup()?;
    let result = App::new(initial)?.run(&mut terminal);
    restore(&mut terminal)?;
    result
}

fn setup() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("create terminal")
}

fn restore(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    Ok(())
}

struct App {
    theme: Theme,
    mode: Mode,
    doc: Option<MetaDoc>,
    tags: Vec<TagView>,
    filtered: Vec<usize>,
    list_state: ListState,
    filter: String,
    input: String,
    message: String,
    presets: Vec<Preset>,
    preset_idx: usize,
}

impl App {
    fn new(initial: Option<PathBuf>) -> Result<Self> {
        let theme = Theme::from_cybercore();
        let mut app = Self {
            theme,
            mode: Mode::Browse,
            doc: None,
            tags: Vec::new(),
            filtered: Vec::new(),
            list_state: ListState::default(),
            filter: String::new(),
            input: String::new(),
            message: String::new(),
            presets: list_presets().to_vec(),
            preset_idx: 0,
        };
        if let Some(path) = initial {
            app.open_path(&path)?;
        } else {
            app.mode = Mode::PathInput;
            app.message = "enter image path".into();
        }
        Ok(app)
    }

    fn open_path(&mut self, path: &Path) -> Result<()> {
        let doc = MetaDoc::open(path)?;
        self.reload_from(doc);
        self.mode = Mode::Browse;
        self.message = format!("opened {}", path.display());
        Ok(())
    }

    fn reload_from(&mut self, doc: MetaDoc) {
        self.tags = doc.tags();
        self.doc = Some(doc);
        self.refilter();
    }

    fn refresh_tags(&mut self) {
        if let Some(doc) = &self.doc {
            self.tags = doc.tags();
            self.refilter();
        }
    }

    fn refilter(&mut self) {
        let q = self.filter.to_ascii_lowercase();
        self.filtered = self
            .tags
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                q.is_empty()
                    || t.name.to_ascii_lowercase().contains(&q)
                    || t.value.to_ascii_lowercase().contains(&q)
                    || t.group.as_str().to_ascii_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            let idx = self.list_state.selected().unwrap_or(0);
            let idx = idx.min(self.filtered.len() - 1);
            self.list_state.select(Some(idx));
        }
    }

    fn selected_tag(&self) -> Option<&TagView> {
        let idx = self.list_state.selected()?;
        let tag_idx = *self.filtered.get(idx)?;
        self.tags.get(tag_idx)
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            if !event::poll(Duration::from_millis(200))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break;
            }
            if self.handle_key(key.code)? {
                break;
            }
        }
        Ok(())
    }

    /// Returns true when the app should quit.
    fn handle_key(&mut self, code: KeyCode) -> Result<bool> {
        match self.mode {
            Mode::PathInput => self.handle_path_input(code),
            Mode::Filter => self.handle_filter(code),
            Mode::Edit => self.handle_edit(code),
            Mode::ConfirmStripAll => self.handle_confirm_strip(code),
            Mode::PresetPicker => self.handle_presets(code),
            Mode::Message => {
                self.mode = if self.doc.is_some() {
                    Mode::Browse
                } else {
                    Mode::PathInput
                };
                Ok(false)
            }
            Mode::Browse => self.handle_browse(code),
        }
    }

    fn handle_browse(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-1),
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
                self.message = "filter".into();
            }
            KeyCode::Char('o') => {
                self.mode = Mode::PathInput;
                self.input.clear();
                self.message = "open path".into();
            }
            KeyCode::Enter => {
                let editable = self
                    .selected_tag()
                    .map(|t| (t.name.clone(), t.value.clone()));
                if let Some((name, value)) = editable {
                    if crate::meta::tags::string_tag_by_name(&name, "").is_ok() {
                        self.input = value;
                        self.mode = Mode::Edit;
                        self.message = format!("edit {name}");
                    } else {
                        self.message = format!("{name} is not a string-editable tag");
                    }
                }
            }
            KeyCode::Char('g') => {
                if let Some(doc) = self.doc.as_mut() {
                    let n = doc.strip_gps();
                    self.refresh_tags();
                    self.message = format!("stripped {n} GPS-related tags (unsaved)");
                }
            }
            KeyCode::Char('X') => {
                self.mode = Mode::ConfirmStripAll;
                self.message = "strip ALL non-structural tags? (y/n)".into();
            }
            KeyCode::Char('p') => {
                self.mode = Mode::PresetPicker;
                self.preset_idx = 0;
                self.message = "presets — Enter apply, Esc cancel".into();
            }
            KeyCode::Char('s') => self.save(false)?,
            KeyCode::Char('S') => self.save(true)?,
            KeyCode::Char('e') => self.export_sidecar()?,
            _ => {}
        }
        Ok(false)
    }

    fn handle_path_input(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Esc => {
                if self.doc.is_some() {
                    self.mode = Mode::Browse;
                } else {
                    return Ok(true);
                }
            }
            KeyCode::Enter => {
                let path = PathBuf::from(self.input.trim());
                match self.open_path(&path) {
                    Ok(()) => self.input.clear(),
                    Err(err) => {
                        self.message = format!("open failed: {err:#}");
                        self.mode = Mode::Message;
                    }
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_filter(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Esc => {
                self.filter.clear();
                self.refilter();
                self.mode = Mode::Browse;
            }
            KeyCode::Enter => {
                self.mode = Mode::Browse;
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.refilter();
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.refilter();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_edit(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                self.input.clear();
            }
            KeyCode::Enter => {
                if let Some(tag) = self.selected_tag().cloned() {
                    if let Some(doc) = self.doc.as_mut() {
                        match doc.set_string_tag(&tag.name, &self.input) {
                            Ok(()) => {
                                self.refresh_tags();
                                self.message = format!("set {} (unsaved)", tag.name);
                            }
                            Err(err) => self.message = format!("edit failed: {err:#}"),
                        }
                    }
                }
                self.mode = Mode::Browse;
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_confirm_strip(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(doc) = self.doc.as_mut() {
                    let n = doc.strip_all();
                    self.refresh_tags();
                    self.message = format!("stripped {n} tags (unsaved)");
                }
                self.mode = Mode::Browse;
            }
            _ => {
                self.mode = Mode::Browse;
                self.message = "strip cancelled".into();
            }
        }
        Ok(false)
    }

    fn handle_presets(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.preset_idx + 1 < self.presets.len() {
                    self.preset_idx += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.preset_idx = self.preset_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                let preset = self.presets[self.preset_idx];
                if let Some(doc) = self.doc.as_mut() {
                    doc.apply_preset(preset)?;
                    self.refresh_tags();
                    self.message = format!("applied preset {} (unsaved)", preset.as_str());
                }
                self.mode = Mode::Browse;
            }
            _ => {}
        }
        Ok(false)
    }

    fn move_sel(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, (self.filtered.len() as i32) - 1) as usize;
        self.list_state.select(Some(next));
    }

    fn save(&mut self, in_place: bool) -> Result<()> {
        let Some(doc) = self.doc.as_mut() else {
            self.message = "no file open".into();
            return Ok(());
        };
        match doc.save(None, in_place) {
            Ok(path) => {
                self.message = if in_place {
                    format!("saved in-place {}", path.display())
                } else {
                    format!("saved {}", path.display())
                };
                self.refresh_tags();
            }
            Err(err) => self.message = format!("save failed: {err:#}"),
        }
        Ok(())
    }

    fn export_sidecar(&mut self) -> Result<()> {
        let Some(doc) = &self.doc else {
            self.message = "no file open".into();
            return Ok(());
        };
        let json = doc.export_string(crate::meta::ExportFormat::Json)?;
        let out = doc.path.with_extension("cybermeta.json");
        std::fs::write(&out, json)?;
        self.message = format!("exported {}", out.display());
        Ok(())
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        self.draw_header(frame, chunks[0]);
        self.draw_body(frame, chunks[1]);
        self.draw_input(frame, chunks[2]);
        self.draw_status(frame, chunks[3]);

        if self.mode == Mode::PresetPicker {
            self.draw_preset_modal(frame, area);
        }
    }

    fn draw_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let dirty = self
            .doc
            .as_ref()
            .map(|d| if d.dirty { " ● dirty" } else { "" })
            .unwrap_or("");
        let path = self
            .doc
            .as_ref()
            .map(|d| d.path.display().to_string())
            .unwrap_or_else(|| "(no file)".into());
        let line = Line::from(vec![
            Span::styled(
                " cybermeta ",
                Style::default()
                    .fg(self.theme.bg)
                    .bg(self.theme.acid_green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {path}{dirty} "),
                Style::default().fg(self.theme.white),
            ),
            Span::styled(
                format!(" theme:{} ", self.theme.name),
                Style::default().fg(self.theme.muted),
            ),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.line))
            .style(Style::default().bg(self.theme.panel));
        frame.render_widget(Paragraph::new(line).block(block), area);
    }

    fn draw_body(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .map(|&i| {
                let t = &self.tags[i];
                let label = format!(
                    "[{}] {:<22} {}",
                    t.group.as_str(),
                    t.name,
                    truncate(&t.value, 40)
                );
                ListItem::new(Span::styled(label, Style::default().fg(self.theme.white)))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" tags ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line))
                    .style(Style::default().bg(self.theme.bg)),
            )
            .highlight_style(
                Style::default()
                    .bg(self.theme.line)
                    .fg(self.theme.acid_green)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(list, panes[0], &mut self.list_state);

        let detail = if let Some(tag) = self.selected_tag() {
            format!(
                "name:  {}\nhex:   0x{:04X}\ngroup: {}\n\nvalue:\n{}",
                tag.name,
                tag.hex,
                tag.group.as_str(),
                tag.value
            )
        } else {
            "no tag selected".into()
        };
        let detail = Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(self.theme.cyan))
            .block(
                Block::default()
                    .title(" detail ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line))
                    .style(Style::default().bg(self.theme.panel)),
            );
        frame.render_widget(detail, panes[1]);
    }

    fn draw_input(&self, frame: &mut ratatui::Frame, area: Rect) {
        let (title, text) = match self.mode {
            Mode::Filter => (" filter ", self.filter.as_str()),
            Mode::Edit => (" edit value ", self.input.as_str()),
            Mode::PathInput => (" path ", self.input.as_str()),
            Mode::ConfirmStripAll => (" confirm ", "strip all? y/n"),
            Mode::PresetPicker => (" presets ", "j/k select · Enter apply"),
            Mode::Browse | Mode::Message => (
                " actions ",
                "j/k move · / filter · Enter edit · g GPS · X all · p preset · s save · S in-place · e export · o open · q quit",
            ),
        };
        let widget = Paragraph::new(Span::styled(
            text.to_string(),
            Style::default().fg(self.theme.hot_pink),
        ))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.theme.line))
                .style(Style::default().bg(self.theme.panel)),
        );
        frame.render_widget(widget, area);
    }

    fn draw_status(&self, frame: &mut ratatui::Frame, area: Rect) {
        let msg = Paragraph::new(Span::styled(
            format!(" {}", self.message),
            Style::default().fg(self.theme.muted),
        ))
        .style(Style::default().bg(self.theme.bg));
        frame.render_widget(msg, area);
    }

    fn draw_preset_modal(&self, frame: &mut ratatui::Frame, area: Rect) {
        let width = 60u16.min(area.width.saturating_sub(4));
        let height = (self.presets.len() as u16 + 2).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let rect = Rect::new(x, y, width, height);
        frame.render_widget(Clear, rect);

        let lines: Vec<Line> = self
            .presets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let marker = if i == self.preset_idx { "▶" } else { " " };
                let style = if i == self.preset_idx {
                    Style::default()
                        .fg(self.theme.acid_green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.white)
                };
                Line::from(Span::styled(
                    format!("{marker} {} — {}", p.as_str(), p.description()),
                    style,
                ))
            })
            .collect();
        let widget = Paragraph::new(lines).block(
            Block::default()
                .title(" spoof / scrub presets ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.theme.hot_pink))
                .style(Style::default().bg(self.theme.panel)),
        );
        frame.render_widget(widget, rect);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

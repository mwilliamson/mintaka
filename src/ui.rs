use std::{sync::{Arc, Mutex}, time::Duration};

use ratatui::{backend::TermwizBackend, buffer::Buffer, layout::{Constraint, Layout, Rect}, style::{Color, Style, Stylize}, text::{Line, Text}, widgets::{Block, List, ListItem, ListState, Widget}, Frame};
use termwiz::{caps::ProbeHints, input::InputEvent, surface::{Change, Surface}, terminal::{buffered::BufferedTerminal, SystemTerminal, Terminal, TerminalWaker}};
use wezterm_term::CellAttributes;

use crate::processes::{ProcessStatus, Processes};



pub(crate) struct MintakaUi {
    terminal: ratatui::Terminal<TermwizBackend>,

    theme: MintakaTheme,
}

impl MintakaUi {
    pub(crate) fn new() -> Self {
        let theme = Self::detect_theme();

        let terminal_capabilities = termwiz::caps::Capabilities::new_with_hints(ProbeHints::new_from_env().mouse_reporting(Some(false))).unwrap();
        let mut terminal = SystemTerminal::new(terminal_capabilities).unwrap();
        terminal.set_raw_mode().unwrap();
        terminal.enter_alternate_screen().unwrap();
        let buffered_terminal = BufferedTerminal::new(terminal).unwrap();

        let terminal = ratatui::Terminal::new(TermwizBackend::with_buffered_terminal(buffered_terminal)).unwrap();

        Self { terminal, theme }
    }

    fn detect_theme() -> MintakaTheme {
        let timeout = Duration::from_millis(100);
        let theme = termbg::theme(timeout);
        match theme {
            Ok(termbg::Theme::Light) | Err(_) => MintakaTheme::Light,
            Ok(termbg::Theme::Dark) => MintakaTheme::Dark,
        }
    }

    pub(crate) fn waker(&mut self) -> TerminalWaker {
        self.terminal.backend_mut().buffered_terminal_mut().terminal().waker()
    }

    pub(crate) fn render(&mut self, processes: &Arc<Mutex<Processes>>) {
        render_ui(processes, &mut self.terminal, self.theme)
    }

    pub(crate) fn poll_input(&mut self) -> Result<Option<termwiz::input::InputEvent>, termwiz::Error> {
        let buffered_terminal = self.terminal.backend_mut().buffered_terminal_mut();
        let input_event = buffered_terminal.terminal().poll_input(None)?;

        if let Some(InputEvent::Resized { rows, cols }) = input_event {
            // FIXME: this is working around a bug where we don't realize
            // that we should redraw everything on resize in BufferedTerminal.
            buffered_terminal.add_change(Change::ClearScreen(Default::default()));
            buffered_terminal.resize(cols, rows);
            Ok(None)
        } else {
            Ok(input_event)
        }
    }
}

#[derive(Clone, Copy)]
enum MintakaTheme {
    Light,
    Dark,
}

impl MintakaTheme {
    fn fg_invert(&self) -> Color {
        match self {
            MintakaTheme::Light => Color::White,
            MintakaTheme::Dark => Color::Black,
        }
    }

    fn bg_invert(&self) -> Color {
        match self {
            MintakaTheme::Light => Color::Black,
            MintakaTheme::Dark => Color::White,
        }
    }

    fn text_style(&self) -> Style {
        Style::default().fg(Color::Reset).bg(Color::Reset)
    }

    fn invert_style(&self) -> Style {
        Style::default().fg(self.fg_invert()).bg(self.bg_invert())
    }
}

/// Render the UI.
///
/// ```
/// +------------+------------------------------------------+
/// |            |                                          |
/// |            |                                          |
/// |            |                                          |
/// |  Process   |               Process                    |
/// |  List      |               Pane                       |
/// |            |                                          |
/// |            |                                          |
/// |            |                                          |
/// +------------+------------------------------------------+
/// | Status Bar                                            |
/// +------------+------------------------------------------+
/// ```
fn render_ui(
    processes: &Arc<Mutex<Processes>>,
    terminal: &mut ratatui::Terminal<TermwizBackend>,
    theme: MintakaTheme,
) {
    let mut processes = processes.lock().unwrap();
    let mut process_pane = ProcessPane::new();

    terminal.draw(|frame| {
        render_chrome(&processes, &mut process_pane, frame, theme);
    }).unwrap();

    let buffered_terminal = terminal.backend_mut().buffered_terminal_mut();
    processes.resize((process_pane.area.width.into(), process_pane.area.height.into()));

    render_process_pane(&processes, &process_pane, buffered_terminal);
}

/// Render the chrome of the UI: that is, render everything except for the
/// actual output of the process.
fn render_chrome(
    processes: &Processes,
    process_pane: &mut ProcessPane,
    frame: &mut Frame,
    theme: MintakaTheme
) {
    let layout = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(frame.size());

    let top_layout = Layout::horizontal([
        Constraint::Length(process_list_width(processes, theme) as u16),
        Constraint::Min(30),
    ]).split(layout[0]);

    render_process_list(processes, top_layout[0], frame, theme);

    render_status_bar(processes, layout[1], frame, theme);

    render_process_pane_placeholder(process_pane, top_layout[1], frame);
}

fn process_list_width(processes: &Processes, theme: MintakaTheme) -> usize {
    let process_labels = process_list_labels(processes, theme);
    let min_label_width = 15;
    let label_width = process_labels
        .map(|label| label.width())
        .max()
        .unwrap_or(min_label_width)
        .max(min_label_width);
    // TODO: Is there a way to calculate this programatically from the block?
    let border_width = 1;
    label_width + border_width * 2
}

fn render_process_list(processes: &Processes, area: Rect, frame: &mut Frame, theme: MintakaTheme) {
    let process_labels = process_list_labels(processes, theme);
    let process_list = List::new(process_labels)
        .block(Block::bordered());
    // TODO: maintain list state
    let mut process_list_state = ListState::default().with_selected(Some(processes.focused_process_index));
    frame.render_stateful_widget(&process_list, area, &mut process_list_state);
}

const STATUS_COLOR_SUCCESS: Color = Color::Green;
const STATUS_COLOR_OTHER: Color = Color::DarkGray;
const STATUS_COLOR_FAILED: Color = Color::Red;

fn process_list_labels(
    processes: & Processes,
    theme: MintakaTheme,
) -> impl Iterator<Item=ListItem> {
    let normal_style = theme.text_style();
    let focused_style = theme.invert_style();

    processes.processes()
        .into_iter()
        .enumerate()
        .map(move |(process_index, process)| {
            let mut text = Text::default();
            let style = if processes.focused_process_index == process_index {
                focused_style
            } else {
                normal_style
            };

            text.push_line(Line::styled(
                format!(" {}. {} ", process_index + 1, process.name()),
                style
            ));

            let (status_str, status_color) = match process.status() {
                ProcessStatus::NotStarted => {
                    ("INACTIVE".to_owned(), STATUS_COLOR_OTHER)
                },
                ProcessStatus::WaitingForUpstream => {
                    ("WAITING".to_owned(), STATUS_COLOR_OTHER)
                },
                ProcessStatus::Running => {
                    ("RUNNING".to_owned(), STATUS_COLOR_OTHER)
                },
                ProcessStatus::Success => {
                    ("SUCCESS".to_owned(), STATUS_COLOR_SUCCESS)
                }
                ProcessStatus::Errors { error_count } => {
                    let mut status_str = "ERR".to_owned();

                    if let Some(error_count) = error_count {
                        let error_count_str = if error_count >= 100 {
                            "99+".to_owned()
                        } else {
                            format!("{error_count}")
                        };
                        status_str.push_str(&format!(" ({error_count_str})"));
                    }

                    (status_str, STATUS_COLOR_FAILED)
                },
                ProcessStatus::Exited { exit_code } => {
                    let status_color = if exit_code == 0 {
                        STATUS_COLOR_SUCCESS
                    } else {
                        STATUS_COLOR_FAILED
                    };
                    (format!("EXIT {exit_code}"), status_color)
                }
            };
            let status_style = style
                .fg(status_color)
                .bold();

            text.push_line(Line::styled(format!("    {status_str}"), status_style));

            ListItem::new(text)
        })
}

fn render_status_bar(
    processes: &Processes,
    area: Rect,
    frame: &mut Frame,
    theme: MintakaTheme,
) {
    let focus_str = if processes.autofocus() {
        "Auto"
    } else {
        "Manual"
    };

    let style = theme.invert_style();

    frame.render_widget(
        Line::styled(format!("  Focus: {focus_str}"), style),
        area,
    );
}

fn render_process_pane_placeholder(process_pane: &mut ProcessPane, area: Rect, frame: &mut Frame) {
    // TODO: render directly?
    frame.render_widget(process_pane, area);
}

fn render_process_pane<T: Terminal>(
    processes: &Processes,
    process_pane: &ProcessPane,
    buffered_terminal: &mut BufferedTerminal<T>,
) {
    let lines = processes.lines();
    let mut process_surface = Surface::new(process_pane.area.width.into(), process_pane.area.height.into());
    process_surface.add_change(Change::ClearScreen(Default::default()));

    for (line_index, line) in lines.iter().enumerate() {
        if line_index != 0 {
            process_surface.add_change(
                termwiz::surface::Change::Text("\r\n".to_owned()),
            );
        }
        let changes = line.changes(&CellAttributes::blank());
        process_surface.add_changes(changes);
        process_surface.add_change(
            termwiz::surface::Change::AllAttributes(CellAttributes::blank()),
        );
    }

    buffered_terminal.draw_from_screen(
        &process_surface,
        process_pane.area.x.into(),
        process_pane.area.y.into(),
    );
    buffered_terminal.flush().unwrap();
}

struct ProcessPane {
    area: Rect,
}

impl ProcessPane {
    fn new() -> Self {
        Self {
            area: Rect::new(0, 0, 0, 0),
        }
    }
}

impl Widget for &mut ProcessPane {
    fn render(self, area: Rect, _buf: &mut Buffer) {
        self.area = area;
    }
}

use std::sync::{Arc, Mutex};

use ratatui::{
    Frame,
    backend::TermwizBackend,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, Paragraph, Widget, Wrap},
};
use termwiz::{
    caps::ProbeHints,
    input::InputEvent,
    surface::{Change, CursorVisibility, Surface},
    terminal::{SystemTerminal, Terminal, TerminalWaker, buffered::BufferedTerminal},
};
use theme::MintakaTheme;
use wezterm_term::{CellAttributes, CursorPosition};

use crate::processes::{MintakaMode, ProcessStatus, Processes, ScreenContents};

mod controls;
mod theme;

pub(crate) use controls::MintakaInputEvent;

pub(crate) struct MintakaUi {
    terminal: ratatui::Terminal<TermwizBackend>,

    process_list_state: ListState,

    theme: MintakaTheme,
}

impl MintakaUi {
    pub(crate) fn new() -> Self {
        let theme = MintakaTheme::detect();

        let terminal_capabilities = termwiz::caps::Capabilities::new_with_hints(
            ProbeHints::new_from_env().mouse_reporting(Some(false)),
        )
        .unwrap();
        let mut terminal = SystemTerminal::new(terminal_capabilities).unwrap();
        terminal.set_raw_mode().unwrap();
        terminal.enter_alternate_screen().unwrap();
        let buffered_terminal = BufferedTerminal::new(terminal).unwrap();

        let terminal =
            ratatui::Terminal::new(TermwizBackend::with_buffered_terminal(buffered_terminal))
                .unwrap();

        let process_list_state = ListState::default();

        Self {
            terminal,
            process_list_state,
            theme,
        }
    }

    pub(crate) fn waker(&mut self) -> TerminalWaker {
        self.terminal
            .backend_mut()
            .buffered_terminal_mut()
            .terminal()
            .waker()
    }

    pub(crate) fn render(&mut self, processes: &Arc<Mutex<Processes>>) {
        render_ui(
            processes,
            &mut self.terminal,
            &mut self.process_list_state,
            self.theme,
        )
    }

    pub(crate) fn poll_input(
        &mut self,
        mode: MintakaMode,
    ) -> Result<Option<MintakaInputEvent>, termwiz::Error> {
        let buffered_terminal = self.terminal.backend_mut().buffered_terminal_mut();
        let input_event = buffered_terminal.terminal().poll_input(None)?;

        match input_event {
            Some(InputEvent::Resized { rows, cols }) => {
                // FIXME: this is working around a bug where we don't realize
                // that we should redraw everything on resize in BufferedTerminal.
                buffered_terminal.add_change(Change::ClearScreen(Default::default()));
                buffered_terminal.resize(cols, rows);
                Ok(None)
            }

            Some(InputEvent::Key(key_event)) => Ok(controls::read_key_event(key_event, mode)),

            _ => Ok(None),
        }
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
    process_list_state: &mut ListState,
    theme: MintakaTheme,
) {
    let mut processes = processes.lock().unwrap();
    let mut process_pane = ProcessPane::new();

    let screen_contents = processes.screen_contents();

    terminal
        .draw(|frame| {
            render_chrome(
                &processes,
                &screen_contents,
                &mut process_pane,
                frame,
                process_list_state,
                theme,
            );
        })
        .unwrap();

    let buffered_terminal = terminal.backend_mut().buffered_terminal_mut();
    processes.resize((
        process_pane.area.width.into(),
        process_pane.area.height.into(),
    ));

    render_process_pane(&screen_contents, &process_pane, buffered_terminal);
}

/// Render the chrome of the UI: that is, render everything except for the
/// actual output of the process.
fn render_chrome(
    processes: &Processes,
    screen_contents: &ScreenContents,
    process_pane: &mut ProcessPane,
    frame: &mut Frame,
    process_list_state: &mut ListState,
    theme: MintakaTheme,
) {
    let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(frame.size());

    let top_layout = Layout::horizontal([
        Constraint::Length(process_list_width(processes, theme) as u16),
        Constraint::Min(30),
    ])
    .split(layout[0]);

    render_process_list(processes, top_layout[0], frame, process_list_state, theme);

    render_status_bar(processes, layout[1], frame, theme);

    render_process_pane_placeholder(screen_contents, process_pane, top_layout[1], frame);
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

fn render_process_list(
    processes: &Processes,
    area: Rect,
    frame: &mut Frame,
    process_list_state: &mut ListState,
    theme: MintakaTheme,
) {
    process_list_state.select(Some(processes.focused_process_index));
    let process_labels = process_list_labels(processes, theme);
    let process_list = List::new(process_labels).block(Block::bordered());
    frame.render_stateful_widget(&process_list, area, process_list_state);
}

const STATUS_COLOR_SUCCESS: Color = Color::Green;
const STATUS_COLOR_OTHER: Color = Color::DarkGray;
const STATUS_COLOR_FAILED: Color = Color::Red;

fn process_list_labels(
    processes: &Processes,
    theme: MintakaTheme,
) -> impl Iterator<Item = ListItem> {
    let normal_style = theme.text_style();
    let focused_style = theme.highlight_style();

    processes
        .processes()
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
                style,
            ));

            let (status_str, status_color) = match process.status() {
                ProcessStatus::NotStarted => ("INACTIVE".to_owned(), STATUS_COLOR_OTHER),
                ProcessStatus::Stopped => ("STOPPED".to_owned(), STATUS_COLOR_OTHER),
                ProcessStatus::WaitingForUpstream => ("WAITING".to_owned(), STATUS_COLOR_OTHER),
                ProcessStatus::FailedToStart => ("ERR".to_owned(), STATUS_COLOR_FAILED),
                ProcessStatus::Running => ("RUNNING".to_owned(), STATUS_COLOR_OTHER),
                ProcessStatus::Success(_) => ("SUCCESS".to_owned(), STATUS_COLOR_SUCCESS),
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
                }
                ProcessStatus::Exited { exit_code } => {
                    let status_color = if exit_code == 0 {
                        STATUS_COLOR_SUCCESS
                    } else {
                        STATUS_COLOR_FAILED
                    };
                    (format!("EXIT {exit_code}"), status_color)
                }
            };
            let status_style = style.fg(status_color).bold();

            text.push_line(Line::styled(format!("    {status_str}"), status_style));

            ListItem::new(text)
        })
}

fn render_status_bar(processes: &Processes, area: Rect, frame: &mut Frame, theme: MintakaTheme) {
    let spans: Vec<_> = controls::describe(processes)
        .iter()
        .flat_map(|(shortcut, description)| {
            [
                Span::styled(*shortcut, theme.text_style()),
                Span::from("  "),
                Span::from(*description),
                Span::from("  "),
            ]
            .into_iter()
        })
        .collect();

    frame.render_widget(Line::from(spans).style(theme.highlight_style()), area);
}

fn render_process_pane_placeholder(
    screen_contents: &ScreenContents,
    process_pane: &mut ProcessPane,
    area: Rect,
    frame: &mut Frame,
) {
    frame.render_widget(process_pane, area);

    match screen_contents {
        ScreenContents::Error(error) => {
            // Since we're rendering terminal output outside of ratatui, we need
            // to clear the area before rendering.
            frame.render_widget(Block::new().bg(Color::Red), area);
            frame.render_widget(Block::new().bg(Color::Reset), area);

            frame.render_widget(
                Paragraph::new(error.to_owned()).wrap(Wrap { trim: false }),
                area,
            );
        }
        ScreenContents::Terminal { .. } => {
            // TODO: render directly?
        }
    }
}

fn render_process_pane<T: Terminal>(
    screen_contents: &ScreenContents,
    process_pane: &ProcessPane,
    buffered_terminal: &mut BufferedTerminal<T>,
) {
    let ScreenContents::Terminal {
        lines,
        cursor_position,
    } = screen_contents
    else {
        return;
    };

    let mut process_surface = Surface::new(
        process_pane.area.width.into(),
        process_pane.area.height.into(),
    );
    process_surface.add_change(Change::ClearScreen(Default::default()));

    for (line_index, line) in lines.iter().enumerate() {
        if line_index != 0 {
            process_surface.add_change(termwiz::surface::Change::Text("\r\n".to_owned()));
        }
        let changes = line.changes(&CellAttributes::blank());
        process_surface.add_changes(changes);
        process_surface.add_change(termwiz::surface::Change::AllAttributes(
            CellAttributes::blank(),
        ));
    }

    buffered_terminal.draw_from_screen(
        &process_surface,
        process_pane.area.x.into(),
        process_pane.area.y.into(),
    );

    render_cursor(&cursor_position, process_pane, buffered_terminal);

    buffered_terminal.flush().unwrap();
}

fn render_cursor<T: Terminal>(
    cursor_position: &Option<CursorPosition>,
    process_pane: &ProcessPane,
    buffered_terminal: &mut BufferedTerminal<T>,
) {
    if let Some((cursor_x, cursor_y)) = cursor_position
        .filter(|cursor_position| match cursor_position.visibility {
            CursorVisibility::Hidden => false,
            CursorVisibility::Visible => true,
        })
        .and_then(|cursor_position| {
            let x = cursor_position.x;
            let y: Option<usize> = cursor_position.y.try_into().ok();
            y.map(|y| (x, y))
        })
    {
        buffered_terminal.add_change(Change::CursorVisibility(CursorVisibility::Visible));
        buffered_terminal.add_change(Change::CursorPosition {
            x: termwiz::surface::Position::Absolute(cursor_x + process_pane.area.x as usize),
            y: termwiz::surface::Position::Absolute(cursor_y + process_pane.area.y as usize),
        });
    } else {
        buffered_terminal.add_change(Change::CursorVisibility(CursorVisibility::Hidden));
    }
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

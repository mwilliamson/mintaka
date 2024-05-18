use std::sync::{Arc, Mutex};

use processes::{ProcessStatus};
use ratatui::{backend::TermwizBackend, buffer::Buffer, layout::{Constraint, Layout, Rect}, style::{Color, Style, Stylize}, text::{Line, Text}, widgets::{Block, List, ListItem, ListState, Widget}, Frame};
use termwiz::{caps::ProbeHints, input::{InputEvent, KeyEvent}, surface::{Change, Surface}, terminal::{buffered::BufferedTerminal, SystemTerminal, Terminal}};
use wezterm_term::{CellAttributes, KeyCode, KeyModifiers};

use crate::processes::Processes;

mod cli;
mod config;
mod processes;

fn main() {
    let config = cli::load_config().unwrap();

    let terminal_capabilities = termwiz::caps::Capabilities::new_with_hints(ProbeHints::new_from_env().mouse_reporting(Some(false))).unwrap();
    let mut terminal = SystemTerminal::new(terminal_capabilities).unwrap();
    terminal.set_raw_mode().unwrap();
    terminal.enter_alternate_screen().unwrap();
    let terminal_waker = terminal.waker();
    let buffered_terminal = BufferedTerminal::new(terminal).unwrap();

    let mut terminal = ratatui::Terminal::new(TermwizBackend::with_buffered_terminal(buffered_terminal)).unwrap();

    let mut processes = Processes::new(terminal_waker);
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }
    let processes = Arc::new(Mutex::new(processes));

    loop {
        {
            let mut processes = processes.lock().unwrap();
            let mut process_pane = ProcessPane::new();
            terminal.draw(|frame| {
                render_main(&processes, &mut process_pane, frame);
            }).unwrap();

            let buffered_terminal = terminal.backend_mut().buffered_terminal_mut();
            processes.resize((process_pane.area.width.into(), process_pane.area.height.into()));

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

        let buffered_terminal = terminal.backend_mut().buffered_terminal_mut();
        match buffered_terminal.terminal().poll_input(None).unwrap() {
            Some(InputEvent::Resized { rows, cols }) => {
                // FIXME: this is working around a bug where we don't realize
                // that we should redraw everything on resize in BufferedTerminal.
                buffered_terminal.add_change(Change::ClearScreen(Default::default()));
                buffered_terminal.resize(cols, rows);
            }
            Some(input) => {
                if let InputEvent::Key(key_event) = input {
                    if matches!(
                        key_event,
                        KeyEvent { key: KeyCode::Char('q'), .. } |
                        KeyEvent { key: KeyCode::Char('c'), modifiers: KeyModifiers::CTRL}
                    ) {
                        return;
                    }

                    match key_event.key {
                        wezterm_term::KeyCode::UpArrow => {
                            let mut processes = processes.lock().unwrap();
                            processes.move_focus_up();
                        },
                        wezterm_term::KeyCode::DownArrow => {
                            let mut processes = processes.lock().unwrap();
                            processes.move_focus_down();
                        },
                        _ => {},
                    }
                }
            },
            None => {}
        }
    }
}

fn render_main(processes: &Processes, process_pane: &mut ProcessPane, frame: &mut Frame) {
    let layout = Layout::horizontal([
        Constraint::Length(process_list_width(processes) as u16),
        Constraint::Min(30),
    ]).split(frame.size());

    render_process_list(processes, layout[0], frame);

    render_process_pane(process_pane, layout[1], frame);
}

fn process_list_width(processes: &Processes) -> usize {
    let process_labels = process_list_labels(processes);
    let min_label_width = 10;
    let label_width = process_labels
        .map(|label| label.width())
        .max()
        .unwrap_or(min_label_width)
        .max(min_label_width);
    // TODO: Is there a way to calculate this programatically from the block?
    let border_width = 1;
    label_width + border_width * 2
}

fn render_process_list(processes: &Processes, area: Rect, frame: &mut Frame) {
    let process_labels = process_list_labels(processes);
    let process_list = List::new(process_labels)
        .block(Block::bordered());
    // TODO: maintain list state
    let mut process_list_state = ListState::default().with_selected(Some(processes.focused_process_index));
    frame.render_stateful_widget(&process_list, area, &mut process_list_state);
}

fn process_list_labels(processes: & Processes) -> impl Iterator<Item=ListItem> {
    let normal_style = Style::default().fg(Color::Black).bg(Color::White);
    let focused_style = Style::default().fg(Color::White).bg(Color::Black);

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
                format!(" {}. {} ", process_index + 1, process.name),
                style
            ));

            let (status_str, status_color) = match process.status {
                ProcessStatus::Ok => {
                    ("OK", Color::Green)
                },
            };
            let status_style = Style::default()
                .fg(status_color)
                .bg(style.bg.unwrap())
                .bold();

            text.push_line(Line::styled(format!("    {status_str}"), status_style));

            ListItem::new(text)
        })
}


fn render_process_pane(process_pane: &mut ProcessPane, area: Rect, frame: &mut Frame) {
    // TODO: render directly?
    frame.render_widget(process_pane, area);
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

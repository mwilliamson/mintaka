use std::sync::{Arc, Mutex};

use ratatui::{backend::TermwizBackend, buffer::Buffer, layout::{Constraint, Layout, Rect}, style::{Color, Style, Stylize}, text::{Line, Text}, widgets::{Block, List, ListItem, ListState, Widget}, Frame};
use termwiz::surface::{Change, Surface};
use wezterm_term::CellAttributes;

use crate::processes::{ProcessStatus, Processes};

pub(crate) fn render_ui(processes: &Arc<Mutex<Processes>>, terminal: &mut ratatui::Terminal<TermwizBackend>) {
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

fn render_main(processes: &Processes, process_pane: &mut ProcessPane, frame: &mut Frame) {
    let layout = Layout::horizontal([
        Constraint::Length(process_list_width(processes) as u16),
        Constraint::Min(30),
    ]).split(frame.size());

    let left_layout = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(layout[0]);

    render_process_list(processes, left_layout[0], frame);

    render_focus(processes, left_layout[1], frame);

    render_process_pane(process_pane, layout[1], frame);
}

fn process_list_width(processes: &Processes) -> usize {
    let process_labels = process_list_labels(processes);
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
                format!(" {}. {} ", process_index + 1, process.name()),
                style
            ));

            let status = process.status();
            let status_color = if status.is_ok() {
                Color::Green
            } else {
                Color::Red
            };
            let status_str = match process.status() {
                ProcessStatus::NotStarted => {
                    "INACTIVE".to_owned()
                },
                ProcessStatus::Running => {
                    "RUNNING".to_owned()
                },
                ProcessStatus::Success => {
                    "SUCCESS".to_owned()
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

                    status_str
                },
                ProcessStatus::Exited { exit_code } => {
                    format!("EXIT {exit_code}")
                }
            };
            let status_style = Style::default()
                .fg(status_color)
                .bg(style.bg.unwrap())
                .bold();

            text.push_line(Line::styled(format!("    {status_str}"), status_style));

            ListItem::new(text)
        })
}

fn render_focus(processes: &Processes, area: Rect, frame: &mut Frame) {
    let focus_str = if processes.autofocus() {
        "Auto"
    } else {
        "Manual"
    };

    frame.render_widget(
        Line::raw(format!("  Focus: {focus_str}")),
        area,
    );
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

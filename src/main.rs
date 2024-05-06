use std::sync::{Arc, Mutex};

use processes::Process;
use termwiz::{caps::ProbeHints, color::AnsiColor, input::{InputEvent, KeyEvent}, surface::{Change, CursorVisibility}, terminal::{buffered::BufferedTerminal, Terminal}, widgets::WidgetEvent};
use wezterm_term::{AttributeChange, CellAttributes, KeyCode, KeyModifiers};

use crate::processes::Processes;

mod cli;
mod config;
mod processes;

fn main() {
    let config = cli::load_config().unwrap();

    let terminal_capabilities = termwiz::caps::Capabilities::new_with_hints(ProbeHints::new_from_env().mouse_reporting(Some(false))).unwrap();
    let mut terminal = termwiz::terminal::new_terminal(terminal_capabilities).unwrap();
    terminal.set_raw_mode().unwrap();
    terminal.enter_alternate_screen().unwrap();
    let terminal_waker = terminal.waker();
    let mut buffered_terminal = BufferedTerminal::new(terminal).unwrap();

    let mut processes = Processes::new(terminal_waker);
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }
    let processes = Arc::new(Mutex::new(processes));

    let mut ui = termwiz::widgets::Ui::new();
    let ui_root_id = ui.set_root(MainScreen);
    let process_list_pane_id = ui.add_child(ui_root_id, ProcessListPane {
        processes: Arc::clone(&processes),
    });
    ui.add_child(ui_root_id, ProcessPane {
        processes,
    });
    ui.set_focus(process_list_pane_id);

    loop {
        ui.process_event_queue().unwrap();

        if ui.render_to_screen(&mut buffered_terminal).unwrap() {
            continue;
        }
        buffered_terminal.flush().unwrap();

        match buffered_terminal.terminal().poll_input(None).unwrap() {
            Some(InputEvent::Resized { rows, cols }) => {
                // FIXME: this is working around a bug where we don't realize
                // that we should redraw everything on resize in BufferedTerminal.
                buffered_terminal.add_change(Change::ClearScreen(Default::default()));
                buffered_terminal.resize(cols, rows);
            }
            Some(input) => {
                if let InputEvent::Key(
                    KeyEvent { key: KeyCode::Char('q'), .. } |
                    KeyEvent { key: KeyCode::Char('c'), modifiers: KeyModifiers::CTRL}
                ) = input {
                    return;
                }
                ui.queue_event(WidgetEvent::Input(input));
            },
            None => {}
        }
    }
}

struct MainScreen;

impl termwiz::widgets::Widget for MainScreen {
    fn render(&mut self, _args: &mut termwiz::widgets::RenderArgs) {
    }

    fn get_size_constraints(&self) -> termwiz::widgets::layout::Constraints {
        let mut constraints = termwiz::widgets::layout::Constraints::default();
        constraints.child_orientation = termwiz::widgets::layout::ChildOrientation::Horizontal;
        constraints
    }
}

struct ProcessListPane {
    processes: Arc<Mutex<Processes>>,
}

impl termwiz::widgets::Widget for ProcessListPane {
    fn render(&mut self, args: &mut termwiz::widgets::RenderArgs) {
        args.cursor.visibility = CursorVisibility::Hidden;
        args.surface.add_change(Change::ClearScreen(Default::default()));
        let processes = self.processes.lock().unwrap();
        for (process_index, process) in processes.processes().into_iter().enumerate() {
            let is_focused = processes.focused_process_index == process_index;

            let (foreground_color, background_color) = if is_focused {
                (AnsiColor::White, AnsiColor::Black)
            } else {
                (AnsiColor::Black, AnsiColor::White)
            };

            args.surface.add_change(Change::Attribute(
                AttributeChange::Background(background_color.into())
            ));
            args.surface.add_change(Change::Attribute(
                AttributeChange::Foreground(foreground_color.into())
            ));

            let process_label = Self::process_label(process_index, &process);
            args.surface.add_change(Change::Text(process_label));

            args.surface.add_change(Change::ClearToEndOfLine(background_color.into()));
            args.surface.add_change(Change::Text("\r\n".to_owned()));
        }
    }

    fn get_size_constraints(&self) -> termwiz::widgets::layout::Constraints {
        let processes = self.processes.lock().unwrap();
        // TODO: .len() is not necessarily the number of cells
        let max_label_len = processes.processes().iter()
            .enumerate()
            .map(|(process_index, process)| Self::process_label(process_index, process).len())
            .max()
            .unwrap_or(30);
        let mut c = termwiz::widgets::layout::Constraints::default();
        c.set_fixed_width(max_label_len as _);
        c
    }

    fn process_event(&mut self, event: &WidgetEvent, _args: &mut termwiz::widgets::UpdateArgs) -> bool {
        match event {
            WidgetEvent::Input(InputEvent::Key(key_event)) => {
                match key_event.key {
                    wezterm_term::KeyCode::UpArrow => {
                        let mut processes = self.processes.lock().unwrap();
                        processes.move_focus_up();
                        true
                    },
                    wezterm_term::KeyCode::DownArrow => {
                        let mut processes = self.processes.lock().unwrap();
                        processes.move_focus_down();
                        true
                    },
                    _ => false,
                }
            },
            _ => false,
        }
    }
}

impl ProcessListPane {
    fn process_label(process_index: usize, process: &Process) -> String {
        format!("{}. {}", process_index + 1, process.name)
    }
}

struct ProcessPane {
    processes: Arc<Mutex<Processes>>,
}

impl termwiz::widgets::Widget for ProcessPane {
    fn render(&mut self, args: &mut termwiz::widgets::RenderArgs) {
        let lines = {
            let mut processes = self.processes.lock().unwrap();
            // TODO: Wait for size before starting processes
            processes.resize(args.surface.dimensions());
            processes.lines()
        };

        args.surface.add_change(Change::ClearScreen(Default::default()));

        for (line_index, line) in lines.iter().enumerate() {
            if line_index != 0 {
                args.surface.add_change(
                    termwiz::surface::Change::Text("\r\n".to_owned()),
                );
            }
            let changes = line.changes(&CellAttributes::blank());
            args.surface.add_changes(changes);
            args.surface.add_change(
                termwiz::surface::Change::AllAttributes(CellAttributes::blank()),
            );
        }
    }
}

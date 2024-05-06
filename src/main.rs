use termwiz::{input::InputEvent, surface::{Change, CursorVisibility}, terminal::Terminal, widgets::WidgetEvent};
use wezterm_term::CellAttributes;

use crate::processes::Processes;

mod cli;
mod config;
mod processes;

fn main() {
    let config = cli::load_config().unwrap();

    let mut processes = Processes::new();
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }

    let terminal_capabilities = termwiz::caps::Capabilities::new_from_env().unwrap();
    let mut terminal = termwiz::terminal::new_terminal(terminal_capabilities).unwrap();
    terminal.set_raw_mode().unwrap();
    terminal.enter_alternate_screen().unwrap();
    let mut buffered_terminal = termwiz::terminal::buffered::BufferedTerminal::new(terminal).unwrap();

    let mut ui = termwiz::widgets::Ui::new();
    let ui_root_id = ui.set_root(MainScreen);
    let process_list_pane_id = ui.add_child(ui_root_id, ProcessListPane {
        processes: &processes,
    });
    ui.add_child(ui_root_id, ProcessPane {
        processes: &processes
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
                ui.queue_event(WidgetEvent::Input(input));
            },
            None => {}
        }

        break;
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

struct ProcessListPane<'a> {
    processes: &'a Processes,
}

impl termwiz::widgets::Widget for ProcessListPane<'_> {
    fn render(&mut self, args: &mut termwiz::widgets::RenderArgs) {
        args.cursor.visibility = CursorVisibility::Hidden;
        args.surface.add_change(Change::ClearScreen(Default::default()));
        for (process_index, process) in self.processes.processes().into_iter().enumerate() {
            args.surface.add_change(Change::Text(format!("{}. ", process_index + 1)));
            args.surface.add_change(Change::Text(process.name.clone()));
            args.surface.add_change(Change::Text("\r\n".to_owned()));
        }
    }

    fn get_size_constraints(&self) -> termwiz::widgets::layout::Constraints {
        let mut c = termwiz::widgets::layout::Constraints::default();
        c.set_halign(termwiz::widgets::layout::HorizontalAlignment::Left);
        c.set_pct_width(20);
        c
    }
}

struct ProcessPane<'a> {
    processes: &'a Processes,
}

impl termwiz::widgets::Widget for ProcessPane<'_> {
    fn render(&mut self, args: &mut termwiz::widgets::RenderArgs) {
        let lines = {
            self.processes.lines()
        };

        args.surface.add_change(Change::ClearScreen(Default::default()));

        for line in lines {
            let changes = line.changes(&CellAttributes::blank());
            args.surface.add_changes(changes);
            args.surface.add_changes(vec![
                termwiz::surface::Change::Text("\r\n".to_owned()),
                termwiz::surface::Change::AllAttributes(CellAttributes::blank()),
            ]);
        }
    }

    fn get_size_constraints(&self) -> termwiz::widgets::layout::Constraints {
        let mut c = termwiz::widgets::layout::Constraints::default();
        c.set_halign(termwiz::widgets::layout::HorizontalAlignment::Right);
        c.set_pct_width(80);
        c
    }
}

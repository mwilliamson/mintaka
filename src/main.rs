use std::{io::{Read, Write}, sync::Arc, time::Duration};

fn main() {
    let pty_system = portable_pty::native_pty_system();

    let pty_pair = pty_system.openpty(portable_pty::PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }).unwrap();

    let mut command = portable_pty::CommandBuilder::new("ls");
    command.cwd(std::env::current_dir().unwrap());
    let mut child_process = pty_pair.slave.spawn_command(command).unwrap();
    std::mem::drop(pty_pair.slave);

    let child_process_writer = pty_pair.master.take_writer().unwrap();

    let terminal_size = wezterm_term::TerminalSize::default();
    let terminal_config = Arc::new(MintakaTerminal);
    let mut terminal = wezterm_term::Terminal::new(
        terminal_size,
        terminal_config,
        "Mintaka",
        "1.0.0",
        Box::new(child_process_writer),
    );

    let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
    let mut child_process_reader = pty_pair.master.try_clone_reader().unwrap();
    std::thread::spawn(move || {
        loop {
            let mut bytes = vec![0; 256];
            let bytes_read = child_process_reader.read(&mut bytes).unwrap();
            if bytes_read == 0 {
                break;
            }
            stdout_tx.send(bytes).unwrap();
        }
    });

    let return_code = child_process.wait().unwrap();
    eprintln!("{return_code}");
    std::mem::drop(pty_pair.master);

    while let Ok(bytes) = stdout_rx.recv() {
        terminal.advance_bytes(bytes);
    }

    let lines = terminal.screen_mut().lines_in_phys_range(0..100);
    let mut stdout = std::io::stdout().lock();
    for line in lines {
        for cell in line.visible_cells() {
            stdout.write_all(cell.as_cell().str().as_bytes()).unwrap();
        }
        stdout.write_all(&[0x0a, 0x0d]).unwrap();
    }
}

#[derive(Debug)]
struct MintakaTerminal;

impl wezterm_term::TerminalConfiguration for MintakaTerminal {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        wezterm_term::color::ColorPalette::default()
    }
}

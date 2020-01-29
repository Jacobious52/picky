use std::{
    io::{stdout, Write},
    time::Duration,
};

use crossterm::{
    cursor::*,
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute, queue,
    style::Print,
    terminal::*,
    Result,
};

pub fn print_events<W>(write: &mut W) -> Result<()>
where
    W: Write,
{
    loop {
        // Wait up to 1s for another event
        if poll(Duration::from_millis(1_000))? {
            // It's guaranteed that read() wont block if `poll` returns `Ok(true)`
            let event = read()?;

            execute!(write, RestorePosition, Print("> "));

            for y in 0..5 {
                execute!(
                    write,
                    MoveToNextLine(1),
                    Print(format!("{}: cats cats cats", y))
                )?;
            }

            if event == Event::Key(KeyCode::Esc.into()) {
                break;
            }
        }
    }

    Ok(())
}

pub fn run() -> Result<()> {
    enable_raw_mode()?;

    for y in 0..5 {
        println!("");
    }

    let mut stdout = stdout();
    execute!(stdout, MoveUp(5), EnableMouseCapture, SavePosition)?;

    if let Err(e) = print_events(&mut stdout) {
        println!("Error: {:?}\r", e);
    }

    execute!(stdout, DisableMouseCapture)?;

    disable_raw_mode()
}

pub fn run2() -> Result<()> {
    let (cols, rows) = size()?;
    // Resize terminal and scroll up.
    execute!(stdout(), SetSize(10, 10), ScrollUp(5))?;

    // Be a good citizen, cleanup
    execute!(stdout(), SetSize(cols, rows))?;
    Ok(())
}

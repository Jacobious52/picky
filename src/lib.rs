use std::{
    fmt::Display,
    io::{stdout, Write},
    time::Duration,
};

use crossterm::{cursor::*, event::*, execute, queue, style::Print, terminal::*, Result};

#[derive(Clone)]
struct Prompt {
    prompt: String,
    text: String,
}

impl Default for Prompt {
    fn default() -> Prompt {
        Prompt {
            prompt: "> ".to_string(),
            text: "".to_string(),
        }
    }
}

fn render<W, T>(prompt: &mut Prompt, write: &mut W, items: &[T]) -> Result<()>
where
    W: Write,
    T: Display,
{
    queue!(
        write,
        RestorePosition,
        Clear(ClearType::UntilNewLine),
        Print(prompt.prompt.clone()),
        Print(prompt.text.clone()),
        MoveRight(prompt.text.len() as u16)
    )?;

    for (y, item) in items.iter().enumerate() {
        queue!(
            write,
            MoveToNextLine(1),
            Print(format!("{}: {}", y + 1, item))
        )?;
    }

    execute!(
        write,
        RestorePosition,
        MoveRight((prompt.prompt.len() + prompt.text.len()) as u16)
    )
}

fn handle_events<W, T>(prompt: &mut Prompt, write: &mut W, items: &[T]) -> Result<()>
where
    W: Write,
    T: Display,
{
    render(prompt, write, items)?;

    loop {
        // Wait up to 1s for another event
        if poll(Duration::from_millis(1_000))? {
            // It's guaranteed that read() wont block if `poll` returns `Ok(true)`
            let event = read()?;

            match event {
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Enter => {
                        break;
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        prompt.text.pop();
                    }
                    KeyCode::Char(c) => {
                        prompt.text.push(c);
                    }
                    _ => {}
                },
                Event::Mouse(_) => {}
                _ => {}
            }
        }
        render(prompt, write, items)?;
    }

    Ok(())
}

pub fn run<T>(items: &[T], height: u16) -> Result<()>
where
    T: Display,
{
    enable_raw_mode()?;

    let (size_cols, size_rows) = size()?;
    let (_, pos_rows) = position()?;

    if pos_rows + height > size_rows {
        queue!(stdout(), ScrollUp(height), MoveUp(height))?;
    }

    // Resize terminal and scroll up.
    queue!(
        stdout(),
        EnableMouseCapture,
        SavePosition,
        SetSize(size_cols, height + 1)
    )?;

    let mut prompt = Prompt::default();

    handle_events(&mut prompt, &mut stdout(), items)?;

    // Be a good citizen, cleanup
    execute!(
        stdout(),
        SetSize(size_cols, size_rows),
        DisableMouseCapture,
        RestorePosition
    )?;

    disable_raw_mode()
}

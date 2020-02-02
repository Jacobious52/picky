use std::{
    collections::HashMap,
    fmt::Display,
    io::{stdout, Write},
    time::Duration,
};

use crossterm::{cursor::*, event::*, execute, queue, style::*, terminal::*, Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;

use rayon::prelude::*;

#[derive(Clone, Debug)]
struct Prompt {
    prompt: String,
    text: String,
    height: u16,
    selection: usize,
}

#[derive(Clone, Debug)]
struct List<T>
where
    T: Display + Clone + Send + Sync,
{
    items: Vec<T>,
}

impl Default for Prompt {
    fn default() -> Prompt {
        Prompt {
            prompt: "> ".to_string(),
            text: "".to_string(),
            height: 5,
            selection: 0,
        }
    }
}

fn render<W, T>(prompt: &mut Prompt, write: &mut W, items: &[T]) -> Result<()>
where
    W: Write,
    T: Display + Clone + Send + Sync,
{
    queue!(
        write,
        RestorePosition,
        Clear(ClearType::UntilNewLine),
        Print(prompt.prompt.clone()),
        Print(prompt.text.clone()),
        MoveRight(prompt.text.len() as u16)
    )?;

    for y in 0..prompt.height {
        queue!(write, MoveToNextLine(1), Clear(ClearType::UntilNewLine))?;
        let y = y as usize;
        if y < items.len() {
            let to_print = &items.get(y).unwrap();

            let mut text = style(format!("{}: {}", y + 1, to_print));
            if y == prompt.selection {
                text = text.yellow().on_blue()
            }

            queue!(write, Print(text))?;
        }
    }

    execute!(
        write,
        RestorePosition,
        MoveRight((prompt.prompt.len() + prompt.text.len()) as u16)
    )
}

fn handle_events<W, T>(prompt: &mut Prompt, write: &mut W, list: &mut List<T>) -> Result<Option<T>>
where
    W: Write,
    T: Display + Clone + Send + Sync + std::fmt::Debug,
{
    let matcher = SkimMatcherV2::default();

    let to_print = list
        .items
        .iter()
        .take(prompt.height as usize)
        .cloned()
        .collect::<Vec<_>>();
    render(prompt, write, &to_print)?;

    let mut ranked: Vec<(i64, &T)> = Vec::with_capacity(20);

    loop {
        // Wait up to 1s for another event
        if poll(Duration::from_millis(1_000))? {
            // It's guaranteed that read() wont block if `poll` returns `Ok(true)`
            let event = read()?;
            let mut changed = false;

            match event {
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Enter => {
                        let ret = list.items.get(prompt.selection).unwrap();
                        return Ok(Some(ret.clone()));
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    KeyCode::Up => {
                        if prompt.selection > 0 {
                            prompt.selection -= 1;
                        } else {
                            prompt.selection = (prompt.height - 1) as usize;
                        }
                    }
                    KeyCode::Down => {
                        if prompt.selection < (prompt.height - 1) as usize {
                            prompt.selection += 1;
                        } else {
                            prompt.selection = 0;
                        }
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        prompt.text.pop();
                        changed = true;
                    }
                    KeyCode::Char(c) => {
                        prompt.text.push(c);
                        changed = true;
                    }
                    _ => {}
                },
                Event::Mouse(_) => {}
                _ => {}
            }

            if changed {
                ranked.clear();

                let scores = list
                    .items
                    .par_iter()
                    .map(|i| {
                        let score = if prompt.text.is_empty() {
                            None
                        } else {
                            matcher.fuzzy_match(&i.to_string(), &prompt.text)
                        };
                        (score, i)
                    })
                    .collect::<Vec<_>>();

                for (score, item) in scores {
                    if let Some(s) = score {
                        ranked.push((s, &item));
                        ranked.sort_by(|a, b| {
                            let sc = b.0.cmp(&a.0);
                            if sc == Ordering::Equal {
                                return b.1.to_string().len().cmp(&a.1.to_string().len()).reverse();
                            }
                            sc
                        });
                        if ranked.len() >= prompt.height as usize {
                            ranked.pop();
                        }
                    }
                }

                execute!(
                    write,
                    MoveToNextLine(30),
                    Print(format!("{}: {:?}", ranked.len(), ranked))
                )?;

                // list.items.par_sort_unstable_by(|a, b| {
                //     let sc = b.score.cmp(&a.score);
                //     if sc == Ordering::Equal {
                //         return b
                //             .item
                //             .to_string()
                //             .len()
                //             .cmp(&a.item.to_string().len())
                //             .reverse();
                //     }
                //     sc
                // });
            }
        }
        let to_print = if ranked.is_empty() {
            list.items
                .iter()
                .take(prompt.height as usize)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            ranked
                .iter()
                .take(prompt.height as usize)
                .map(|i| i.1)
                .cloned()
                .collect::<Vec<_>>()
        };
        render(prompt, write, &to_print)?;
    }

    Ok(None)
}

pub fn run<T>(items: &[T], height: u16) -> Result<Option<T>>
where
    T: Display + Clone + Send + Sync + std::fmt::Debug,
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

    let mut prompt = Prompt {
        height: height as u16,
        ..Prompt::default()
    };
    let mut list = List {
        items: items.iter().map(|v| v).collect(),
    };

    let result = handle_events(&mut prompt, &mut stdout(), &mut list)?;

    // clean up

    execute!(
        stdout(),
        SetSize(size_cols, size_rows),
        DisableMouseCapture,
        RestorePosition
    )?;

    disable_raw_mode()?;

    Ok(result.cloned())
}

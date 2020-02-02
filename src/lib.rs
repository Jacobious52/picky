use std::{
    collections::BinaryHeap,
    fmt::{Debug, Display},
    io::{stdout, Write},
    time::Duration,
};

use crossterm::{cursor::*, event::*, execute, queue, style::*, terminal::*, Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;

use rayon::prelude::*;

pub trait Item: Display + Debug + Clone + Send + Sync + PartialEq {}
impl<T: Display + Debug + Clone + Send + Sync + PartialEq> Item for T {}

#[derive(Clone, Debug)]
struct Prompt {
    prompt: String,
    text: String,
    height: u16,
    selection: usize,
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
    T: Item,
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

#[derive(Debug, Clone)]
struct RankedItem<T>(T, i64)
where
    T: Item;

impl<T> Ord for RankedItem<T>
where
    T: Item,
{
    fn cmp(&self, other: &Self) -> Ordering {
        let ord = self.1.cmp(&other.1);
        if ord == Ordering::Equal {
            return self
                .0
                .to_string()
                .len()
                .cmp(&other.0.to_string().len())
                .reverse();
        }
        ord
    }
}

impl<T> Eq for RankedItem<T> where T: Item {}

impl<T> PartialOrd for RankedItem<T>
where
    T: Item,
{
    fn partial_cmp(&self, other: &RankedItem<T>) -> Option<Ordering> {
        self.1.partial_cmp(&other.1)
    }
}

impl<T> PartialEq for RankedItem<T>
where
    T: Item,
{
    fn eq(&self, other: &RankedItem<T>) -> bool {
        self.1 == other.1
    }
}

fn score_items<T>(matcher: &SkimMatcherV2, items: &[T], query: &str) -> Vec<RankedItem<T>>
where
    T: Item,
{
    items
        .par_iter()
        .filter_map(|i| {
            let score = matcher.fuzzy_match(&i.to_string(), query);
            if let Some(s) = score {
                return Some(RankedItem(i.clone(), s));
            }
            None
        })
        .collect::<Vec<_>>()
}

fn rank_items<T>(scored: &[RankedItem<T>], max: usize) -> BinaryHeap<RankedItem<T>>
where
    T: Item,
{
    let mut heap = BinaryHeap::with_capacity(max);

    for item in scored.iter() {
        heap.push(item.clone());
    }

    heap
}

fn handle_events<W, T>(prompt: &mut Prompt, write: &mut W, list: &[T]) -> Result<Option<T>>
where
    W: Write,
    T: Item,
{
    let matcher = SkimMatcherV2::default();
    let mut ranked: BinaryHeap<RankedItem<T>> = BinaryHeap::new();

    let to_print = list
        .iter()
        .take(prompt.height as usize)
        .cloned()
        .collect::<Vec<_>>();
    render(prompt, write, &to_print)?;

    loop {
        if poll(Duration::from_millis(1_000))? {
            let event = read()?;
            let mut changed = false;

            match event {
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Enter => {
                        let top: Vec<T> = if prompt.text.is_empty() {
                            list.iter().take(prompt.selection + 1).cloned().collect()
                        } else {
                            ranked
                                .into_iter()
                                .take(prompt.selection + 1)
                                .map(|r| r.0)
                                .collect()
                        };

                        let selected = top.last();
                        return Ok(selected.cloned());
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
                let scored = score_items(&matcher, list, &prompt.text);
                ranked = rank_items(&scored, prompt.height as usize);

                execute!(
                    write,
                    MoveToNextLine(25),
                    Clear(ClearType::UntilNewLine),
                    Print(format!(
                        "{}: {:?}",
                        ranked.len(),
                        ranked
                            .iter()
                            .take(prompt.height as usize)
                            .collect::<Vec<_>>(),
                    ))
                )?;
            }
        }

        let to_print = if prompt.text.is_empty() {
            list.iter()
                .take(prompt.height as usize)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            ranked
                .iter()
                .take(prompt.height as usize)
                .map(|i| i.0.clone())
                .collect::<Vec<_>>()
        };
        render(prompt, write, &to_print)?;
    }

    Ok(None)
}

pub fn run<T>(items: &[T], height: u16) -> Result<Option<T>>
where
    T: Item,
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
    let result = handle_events(&mut prompt, &mut stdout(), items)?;

    // clean up

    execute!(
        stdout(),
        SetSize(size_cols, size_rows),
        DisableMouseCapture,
        RestorePosition,
        Clear(ClearType::UntilNewLine)
    )?;

    disable_raw_mode()?;

    Ok(result)
}

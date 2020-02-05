use std::{
    collections::BinaryHeap,
    collections::HashMap,
    fmt::{Debug, Display},
    io::{stdout, Write},
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{cursor::*, event::*, execute, queue, style::*, terminal::*, Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;

use rayon::prelude::*;

pub trait Item: Display + Clone + Send + Sync {}
impl<T: Display + Clone + Send + Sync> Item for T {}

#[derive(Clone, Debug)]
struct Prompt {
    prompt: String,
    text: String,
    header: Option<String>,
    height: usize,
    width: usize,
    selection: usize,
}

impl Default for Prompt {
    fn default() -> Prompt {
        Prompt {
            prompt: "> ".to_string(),
            text: "".to_string(),
            header: None,
            width: 20,
            height: 5,
            selection: 0,
        }
    }
}

fn render<W, T>(prompt: &Prompt, write: &mut W, items: &[RankedItem<T>]) -> Result<()>
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

    if let Some(header) = prompt.header.clone() {
        queue!(
            write,
            MoveToNextLine(1),
            Clear(ClearType::UntilNewLine),
            MoveRight(3),
            Print(style(header).dark_green()),
        )?;
    }

    for y in 0..prompt.height {
        queue!(write, MoveToNextLine(1), Clear(ClearType::UntilNewLine))?;
        if y < items.len() {
            let to_print = &items.get(y).unwrap();

            let item_string = to_print.0.clone().to_string();
            let to_display = if item_string.len() > prompt.width {
                &item_string[..prompt.width - 3]
            } else {
                &item_string[..]
            };

            let chars = &to_print.2;
            let mut styled = Vec::with_capacity(to_display.len());
            for (i, c) in to_display.chars().enumerate() {
                styled.push(if chars.contains(&i) && to_print.1.is_some() {
                    style(c).magenta().underlined().bold().italic()
                } else {
                    style(c)
                });
            }

            let num = style(format!("{}", y + 1)).blue();
            let mut delim = style(": ").blue();
            if y == prompt.selection {
                delim = style("> ").red().bold();
                styled = styled.into_iter().map(|s| s.yellow().on_blue()).collect();
            }

            queue!(write, Print(num), Print(delim))?;

            for style in styled {
                queue!(write, Print(style))?;
            }
        }
    }

    execute!(
        write,
        RestorePosition,
        MoveRight((prompt.prompt.len() + prompt.text.len()) as u16)
    )
}

#[derive(Debug, Clone)]
struct RankedItem<T>(Arc<T>, Option<i64>, Vec<usize>)
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

fn score_items<T>(matcher: &SkimMatcherV2, items: &mut [RankedItem<T>], query: &str)
where
    T: Item,
{
    items.par_iter_mut().for_each(|i| {
        let result = matcher.fuzzy_indices(&i.0.to_string(), query);
        if let Some((score, indices)) = result {
            i.1 = Some(score);
            i.2 = indices;
        } else {
            i.1 = None;
        }
    });
}

fn rank_items<T>(scored: &[RankedItem<T>], heap: &mut BinaryHeap<RankedItem<T>>)
where
    T: Item,
{
    heap.clear();
    scored
        .iter()
        .filter(|r| r.1.is_some())
        .for_each(|r| heap.push(r.clone()));
}

fn handle_events<W, T>(
    prompt: &mut Prompt,
    write: &mut W,
    list: &mut [RankedItem<T>],
) -> Result<Option<T>>
where
    W: Write,
    T: Item,
{
    let matcher = SkimMatcherV2::default();
    let mut ranked: BinaryHeap<RankedItem<T>> = BinaryHeap::with_capacity(list.len());

    let mut cache: HashMap<String, Vec<RankedItem<T>>> = HashMap::new();
    let mut background_cache: Vec<_> = "abcdefghijklmnopqrstuvwxyzABCDEFGIJKLMNOPQRSTUVWXYZ"
        .chars()
        .rev()
        .collect();

    let to_print = list.iter().take(prompt.height).cloned().collect::<Vec<_>>();
    render(prompt, write, &to_print)?;

    loop {
        if poll(Duration::from_millis(500))? {
            let event = read()?;
            let mut changed = false;
            let _now = Instant::now();

            match event {
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Enter => {
                        let top: Vec<T> = if prompt.text.is_empty() {
                            list.par_iter()
                                .take(prompt.selection + 1)
                                .map(|r| (*r.0).clone())
                                .collect()
                        } else {
                            ranked
                                .par_iter()
                                .take(prompt.selection + 1)
                                .map(|r| (*r.0).clone())
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
                            prompt.selection = prompt.height - 1;
                        }
                    }
                    KeyCode::Down => {
                        if prompt.selection < prompt.height - 1 {
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

            if changed && !prompt.text.is_empty() && !cache.contains_key(&prompt.text) {
                score_items(&matcher, list, &prompt.text);
                rank_items(&list, &mut ranked);
            }

        //prompt.prompt = format!("{}ms> ", now.elapsed().as_millis());
        } else {
            // background cache
            // if let Some(next) = background_cache.pop() {
            //     score_items(&matcher, list, &next.to_string());
            //     rank_items(&list, &mut ranked);
            //     cache.insert(
            //         next.to_string(),
            //         ranked
            //             .iter()
            //             .take(prompt.height)
            //             .cloned()
            //             .collect::<Vec<_>>(),
            //     );
            // }
            continue;
        }

        let query = prompt.text.clone();
        if !query.is_empty() {
            if let Some(cached) = cache.get(&query) {
                render(prompt, write, &cached)?;
                continue;
            }
        }

        let to_print = if query.is_empty() {
            list.par_iter()
                .take(prompt.height)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            ranked
                .par_iter()
                .take(prompt.height)
                .cloned()
                .collect::<Vec<_>>()
        };

        cache.insert(query, to_print.to_vec());
        render(prompt, write, &to_print)?;
    }

    Ok(None)
}

pub fn run<T>(items: &[T], height: u16, header: Option<&str>) -> Result<Option<T>>
where
    T: Item,
{
    enable_raw_mode()?;

    let final_height = if header.is_none() { height } else { height + 1 };

    let (size_cols, size_rows) = size()?;
    let (_, pos_rows) = position()?;

    if pos_rows + final_height > size_rows {
        queue!(stdout(), ScrollUp(final_height), MoveUp(final_height))?;
    }

    // Resize terminal and scroll up.
    queue!(
        stdout(),
        EnableMouseCapture,
        MoveToColumn(1),
        SavePosition,
        SetSize(size_cols, final_height)
    )?;

    let mut prompt = Prompt {
        height: height as usize,
        width: size_cols as usize,
        header: header.map(|s| s.into()),
        ..Prompt::default()
    };

    let mut list = items
        .par_iter()
        .map(|i| RankedItem(Arc::new(i), None, Vec::new()))
        .collect::<Vec<_>>();

    let result = handle_events(&mut prompt, &mut stdout(), &mut list)?;

    // clean up

    queue!(stdout(), RestorePosition)?;
    for _ in 0..final_height {
        queue!(stdout(), MoveToNextLine(1), Clear(ClearType::UntilNewLine))?;
    }

    execute!(
        stdout(),
        SetSize(size_cols, size_rows),
        DisableMouseCapture,
        RestorePosition,
        Clear(ClearType::UntilNewLine)
    )?;

    disable_raw_mode()?;

    Ok(result.cloned())
}

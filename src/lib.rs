use std::{
    cmp::Ordering,
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
use rand::Rng;
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
    color_map: HashMap<char, Color>,
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
            color_map: HashMap::new(),
        }
    }
}

fn render<W, T>(prompt: &Prompt, write: &mut W, items: &[RankedItem<T>]) -> Result<()>
where
    W: Write,
    T: Item,
{
    let styled_prompt: Vec<_> = prompt
        .text
        .chars()
        .map(|c| style(c).with(*prompt.color_map.get(&c).unwrap_or(&Color::White)))
        .collect();

    queue!(
        write,
        RestorePosition,
        Clear(ClearType::UntilNewLine),
        Print(style(prompt.prompt.clone()).cyan().slow_blink().bold()),
    )?;

    for style in styled_prompt {
        queue!(write, Print(style))?;
    }

    queue!(write, MoveRight(prompt.text.len() as u16))?;

    if let Some(header) = prompt.header.clone() {
        let header_trimmed = if header.len() > prompt.width {
            &header[..prompt.width - 3]
        } else {
            &header[..]
        };
        queue!(
            write,
            MoveToNextLine(1),
            Clear(ClearType::UntilNewLine),
            MoveRight(3),
            Print(style(header_trimmed).dark_green()),
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

            let matched_chars = &to_print.2;
            let styled: Vec<_> = to_display
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    let mut s = style(c);
                    if y == prompt.selection {
                        s = s.on_dark_grey();
                    }
                    if matched_chars.contains(&i) && to_print.1.is_some() {
                        s = s
                            .with(*prompt.color_map.get(&c).unwrap_or(&Color::White))
                            .underlined()
                            .bold()
                            .italic();
                    }
                    s
                })
                .collect();

            let num = style(format!("{}", y + 1)).blue();
            let delim = if y == prompt.selection {
                style("> ").red().bold()
            } else {
                style(": ").blue()
            };

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

impl<T> RankedItem<T>
where
    T: Item,
{
    fn rank(&mut self, matcher: &SkimMatcherV2, query: &str) {
        let result = matcher.fuzzy_indices(&self.0.to_string(), query);
        if let Some((score, indices)) = result {
            self.1 = Some(score);
            self.2 = indices;
        } else {
            self.1 = None;
        }
    }
}

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
    items.par_iter_mut().for_each(|item| {
        item.rank(matcher, query);
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
            if let Some(next) = background_cache.pop() {
                let mut list_clone = list.to_vec();
                let mut ranked_clone = BinaryHeap::with_capacity(list_clone.len());
                score_items(&matcher, &mut list_clone, &next.to_string());
                rank_items(&list_clone, &mut ranked_clone);
                cache.insert(
                    next.to_string(),
                    ranked_clone
                        .iter()
                        .take(prompt.height)
                        .cloned()
                        .collect::<Vec<_>>(),
                );
            }
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

// TODO: turn into builder with options
pub fn run<T>(items: &[T], height: u16, header: Option<&str>, resize: bool) -> Result<Option<T>>
where
    T: Item,
{
    enable_raw_mode()?;
    let mut rng = rand::thread_rng();

    let final_height = if header.is_none() {
        height + 1
    } else {
        height + 2
    };

    let (size_cols, size_rows) = size()?;
    let (_, pos_rows) = position()?;

    if pos_rows + final_height > size_rows {
        queue!(stdout(), ScrollUp(final_height), MoveUp(final_height))?;
    }

    // Resize terminal and scroll up.
    queue!(stdout(), EnableMouseCapture, MoveToColumn(1), SavePosition)?;

    if resize {
        queue!(stdout(), SetSize(size_cols, final_height))?;
    }

    let mut prompt = Prompt {
        height: height as usize,
        width: size_cols as usize,
        header: header.map(|s| s.into()),
        color_map: "abcdefghijklmnopqrstuvwxyzABCDEFGIJKLMNOPQRSTUVWXYZ"
            .chars()
            .map(|c| (c, Color::AnsiValue(rng.gen())))
            .collect(),
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

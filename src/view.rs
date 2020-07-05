use std::io::Write;
use std::time::Duration;

use crossterm::{cursor::*, event::*, execute, queue, style::*, terminal::*};

use crate::{model, Handler};

pub struct View<'a, W: Write> {
    writer: &'a mut W,

    prompt: Prompt,
    list: List,

    window_height: u16,
    header: bool,

    handler: Box<dyn Handler>,
}

impl<'a, W> View<'a, W>
where
    W: Write,
{
    pub fn new(
        writer: &'a mut W,
        handler: Box<dyn Handler>,
        table: model::SharedTable,
        window_height: u16,
        header: bool,
    ) -> Self {
        Self {
            window_height,
            header,
            writer,

            prompt: Prompt {
                prompt_text: String::from("> "),
                query_text: String::from(""),
            },
            list: List {
                table,
                height: window_height,
            },

            handler,
        }
    }

    pub fn show(&mut self) -> crossterm::Result<()> {
        enable_raw_mode()?;

        // compute height of window including header
        let final_height = if self.header {
            self.window_height + 1
        } else {
            self.window_height + 2
        };

        let (terminal_size_cols, terminal_size_rows) = size()?;
        let (_, cursor_starting_pos) = position()?;

        if cursor_starting_pos + final_height > terminal_size_rows {
            queue!(self.writer, ScrollUp(final_height), MoveUp(final_height))?;
        }

        // Resize terminal and scroll up.
        queue!(
            self.writer,
            EnableMouseCapture,
            MoveToColumn(1),
            SavePosition
        )?;

        // run until done
        let _ = self.event_loop()?;

        // clean up
        queue!(self.writer, RestorePosition)?;
        for _ in 0..final_height {
            queue!(
                self.writer,
                MoveToNextLine(1),
                Clear(ClearType::UntilNewLine)
            )?;
        }

        execute!(
            self.writer,
            SetSize(terminal_size_cols, terminal_size_rows),
            DisableMouseCapture,
            RestorePosition,
            Clear(ClearType::UntilNewLine)
        )?;

        disable_raw_mode()?;

        Ok(())
    }

    fn event_loop(&mut self) -> crossterm::Result<()> {
        self.render()?;

        loop {
            if poll(Duration::from_millis(500))? {
                let event = read()?;
                let mut changed = false;

                match event {
                    Event::Key(KeyEvent { code, .. }) => match code {
                        KeyCode::Enter => {}
                        KeyCode::Esc => {
                            break;
                        }
                        KeyCode::Up => {}
                        KeyCode::Down => {}
                        KeyCode::Delete | KeyCode::Backspace => {
                            self.prompt.query_text.pop();
                            changed = true;
                        }
                        KeyCode::Char(c) => {
                            self.prompt.query_text.push(c);
                            changed = true;
                        }
                        _ => {}
                    },
                    Event::Mouse(_) => {}
                    _ => {}
                }

                if changed {
                    self.handler.update(&self.prompt.query_text);
                }
            }

            self.render()?;
        }

        Ok(())
    }

    fn render(&mut self) -> crossterm::Result<()> {
        self.prompt.render(&mut self.writer)?;
        self.list.render(&mut self.writer)?;

        execute!(
            self.writer,
            RestorePosition,
            MoveRight((self.prompt.prompt_text.len() + self.prompt.query_text.len()) as u16)
        )?;

        Ok(())
    }
}

pub struct Prompt {
    prompt_text: String,
    query_text: String,
}

impl Prompt {
    fn render<W: Write>(&mut self, writer: &mut W) -> crossterm::Result<()> {
        queue!(
            writer,
            RestorePosition,
            Clear(ClearType::UntilNewLine),
            Print(style(self.prompt_text.clone()).cyan().slow_blink().bold()),
        )?;

        queue!(writer, Print(self.query_text.clone()))?;

        queue!(writer, MoveRight(self.query_text.len() as u16))?;

        Ok(())
    }
}

pub struct List {
    table: model::SharedTable,
    height: u16,
}

impl List {
    fn render<W: Write>(&mut self, writer: &mut W) -> crossterm::Result<()> {
        let h = &self.table.borrow_mut().ranked.clone();
        for y in 0..self.height {
            let y = y as usize;
            queue!(writer, MoveToNextLine(1), Clear(ClearType::UntilNewLine))?;

            if y < self.table.clone().borrow().rows.len() {
                let ri = &self.table.borrow_mut().ranked.pop();
                if ri.is_none() {
                    continue;
                }
                let to_print = &self.table.borrow().rows[ri.as_ref().unwrap().index];
                queue!(
                    writer,
                    Print(format!("{} ", ri.as_ref().map_or(0, |ri| ri.score))),
                    Print(to_print.cols.join("\t"))
                )?;
            }
        }
        self.table.borrow_mut().ranked.extend(h);
        Ok(())
    }
}

use std::{cell::RefCell, io::Write, rc::Rc};

mod model;
mod view;

pub struct Picker {
    window_height: u16,
}

impl Default for Picker {
    fn default() -> Picker {
        Picker { window_height: 5 }
    }
}

impl Picker {
    pub fn run<W: Write>(&self, writer: &mut W, input: &[&str]) -> crossterm::Result<()> {
        let table = Rc::new(RefCell::new(model::Table::new(input)));
        let ranker = model::Ranker::new(table.clone());

        let mut view = view::View::new(writer, Box::new(ranker), table, self.window_height, false);
        view.show()?;

        Ok(())
    }
}

pub trait Handler {
    fn update(&mut self, query: &str);
}

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use std::collections::BinaryHeap;
use std::{cell::RefCell, cmp::Ordering, rc::Rc};

use crate::Handler;

pub struct Row {
    pub cols: Vec<String>,
}

impl Row {
    pub fn new(s: &str) -> Self {
        Self {
            cols: s.split_whitespace().map(String::from).collect(),
        }
    }

    fn compute_score(&self, matcher: &SkimMatcherV2, query: &str) -> Option<i64> {
        matcher.fuzzy_match(&self.cols.join(" "), query)
    }
}

#[derive(Copy, Clone)]
pub struct RankedRowIndex {
    pub index: usize,
    pub score: i64,
}

impl Ord for RankedRowIndex {
    fn cmp(&self, other: &Self) -> Ordering {
        let ord = self.score.cmp(&other.score);
        if ord == Ordering::Equal {
            return self.index.cmp(&other.index);
        }
        ord
    }
}

impl Eq for RankedRowIndex {}

impl PartialOrd for RankedRowIndex {
    fn partial_cmp(&self, other: &RankedRowIndex) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl PartialEq for RankedRowIndex {
    fn eq(&self, other: &RankedRowIndex) -> bool {
        self.score == other.score
    }
}

pub struct Table {
    pub rows: Vec<Row>,
    pub ranked: BinaryHeap<RankedRowIndex>,
}

impl Table {
    pub fn new(rows: &[&str]) -> Self {
        Self {
            rows: rows.iter().map(|r| Row::new(r)).collect(),
            ranked: BinaryHeap::with_capacity(rows.len()),
        }
    }
}

pub type SharedTable = Rc<RefCell<Table>>;

pub struct Ranker {
    table: SharedTable,
    matcher: SkimMatcherV2,
}

impl Ranker {
    pub fn new(table: SharedTable) -> Self {
        Self {
            table,
            matcher: SkimMatcherV2::default(),
        }
    }
}

impl Handler for Ranker {
    fn update(&mut self, query: &str) {
        let ri = {
            let mut table = self.table.borrow_mut();
            table.ranked.clear();
            table
                .rows
                .iter_mut()
                .enumerate()
                .filter_map(|(i, row)| {
                    let score = row.compute_score(&self.matcher, query);
                    score?;
                    Some(RankedRowIndex {
                        score: score.unwrap(),
                        index: i,
                    })
                })
                .collect::<Vec<_>>()
        };
        ri.iter()
            .for_each(|ri| self.table.borrow_mut().ranked.push(*ri));
    }
}

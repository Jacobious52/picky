use picky;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn main() {
    let lines = match read_lines("/usr/share/dict/words") {
        Ok(it) => it,
        _ => return,
    };
    let mut words = Vec::new();

    for line in lines {
        if let Ok(ip) = line {
            words.push(ip);
        }
    }
    let result = picky::run(&words, 20, None).unwrap();
    if let Some(result) = result {
        println!("{}", result);
    }
}

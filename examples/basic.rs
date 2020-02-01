use picky;

fn main() {
    picky::run(&vec!["cats", "dogs", "bears"].to_vec(), 3).unwrap();
}

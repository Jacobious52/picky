use picky;

fn main() {
    let result = picky::run(&["dogs", "cats", "mice", "bears", "sheep"], 10, None).unwrap();
    if let Some(result) = result {
        println!("{}", result);
    }
}

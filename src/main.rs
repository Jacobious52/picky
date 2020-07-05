use picky::Picker;

fn main() {
    let input = vec!["cat", "dog", "fish", "bear", "soup"];

    let picker = Picker::default();
    picker.run(&mut std::io::stdout(), &input).unwrap();
}

use std::process::Command;
use std::str;

use picky;

fn main() {
    let output = Command::new("ps").arg("aux").output().unwrap();

    if !output.status.success() {
        panic!("Command executed with failing error code");
    }

    let lines: Vec<_> = str::from_utf8(&output.stdout).unwrap().lines().collect();

    let result = picky::run(&lines[1..], 3, Some(&lines[0]), true).unwrap();
    if let Some(result) = result {
        println!("{}", result);
    }
}

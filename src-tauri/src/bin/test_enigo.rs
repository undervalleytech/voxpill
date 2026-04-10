use enigo::{Enigo, Key, Keyboard, Settings, Direction};
fn main() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Unicode('c'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();
    println!("Done Unicode");
}

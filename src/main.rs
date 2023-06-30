mod green;

fn ortag() {
    for _ in 0..10 {
        println!("Ortag");
        green::schedule();
    }
}

fn mash() {
    green::spawn(ortag, 2 * 1024 * 1024);
    for _ in 0..10 {
        println!("Mash!");
        green::schedule();
    }
}

fn gaia() {
    green::spawn(mash, 2 * 1024 * 1024);
    for _ in 0..10 {
        println!("Gaia!");
        green::schedule();
    }
}

fn main() {
    green::spawn(gaia, 2 * 1024 * 1024);
}

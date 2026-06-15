fn print_banner() {
    let banner = include_str!("../assets/banner.txt");
    println!("{banner}");
}

fn main() {
    print_banner();

    println!("Starting mercurius-p...");
}
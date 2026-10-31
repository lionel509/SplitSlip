fn main() {
    std::process::exit(splitslip::cli::run(std::env::args().skip(1).collect()))
}

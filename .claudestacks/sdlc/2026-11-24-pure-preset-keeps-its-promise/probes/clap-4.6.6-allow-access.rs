// Throwaway probe: the error clap 4.6.6 prints for a repeatable flag whose values come from a
// library-owned list (no CLI-side mirror enum).
use clap::Parser;
use clap::builder::{PossibleValuesParser, TypedValueParser as _};

const NAMES: [&str; 7] = ["stdin", "stdout", "stderr", "clock", "cwd", "require", "zones"];

#[derive(Debug, Parser)]
#[command(name = "airsl")]
struct Cli {
    /// Host access the sandbox may use. Repeat for several.
    #[arg(long = "allow-access", value_name = "NAME",
          value_parser = PossibleValuesParser::new(NAMES).map(|s| s))]
    access: Vec<String>,
}

fn main() {
    match Cli::try_parse() {
        Ok(cli) => println!("ok {:?}", cli.access),
        Err(e) => { print!("{}", e.render()); println!("exit={}", e.exit_code()); }
    }
}

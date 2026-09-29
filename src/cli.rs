//! Command-line options.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: smallrogue [--seed N] [--bot]
       smallrogue --simulate RUNS [--seed FIRST] [--csv FILE]

  --seed N          play the dungeon for seed N (with --simulate: the
                    first seed of the batch, default 1)
  --bot             let the bot play; press Escape or B to take over
  --simulate RUNS   let the bot play RUNS games without drawing, then
                    print a summary (use a --release build for speed)
  --csv FILE        where --simulate saves one row per run
                    (default target/sim.csv)";

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub seed: Option<u64>,
    pub bot: bool,
    pub simulate: Option<u64>,
    pub csv: PathBuf,
}

/// Parses the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        seed: None,
        bot: false,
        simulate: None,
        csv: PathBuf::from("target/sim.csv"),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        // Flags that take a value read it from the next argument.
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} needs a value\n\n{USAGE}"))
        };
        let number = |name: &str, text: &str| {
            text.parse::<u64>()
                .map_err(|_| format!("{name} expects a whole number, not '{text}'"))
        };
        match arg.as_str() {
            "--seed" => options.seed = Some(number("--seed", value("--seed")?)?),
            "--simulate" => {
                let runs = number("--simulate", value("--simulate")?)?;
                if runs == 0 {
                    return Err("--simulate needs at least 1 run".to_string());
                }
                options.simulate = Some(runs);
            }
            "--csv" => options.csv = PathBuf::from(value("--csv")?),
            "--bot" => options.bot = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option '{other}'\n\n{USAGE}")),
        }
    }
    if options.bot && options.simulate.is_some() {
        return Err("use either --bot or --simulate, not both".to_string());
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn plain_start() {
        let o = parse(&[]).unwrap();
        assert_eq!((o.seed, o.bot, o.simulate), (None, false, None));
    }

    #[test]
    fn all_options() {
        let o = parse(&args("--seed 42 --bot")).unwrap();
        assert_eq!((o.seed, o.bot), (Some(42), true));
        let o = parse(&args("--simulate 50 --seed 7 --csv out.csv")).unwrap();
        assert_eq!(o.simulate, Some(50));
        assert_eq!(o.seed, Some(7));
        assert_eq!(o.csv, PathBuf::from("out.csv"));
    }

    #[test]
    fn mistakes_explain_themselves() {
        assert!(
            parse(&args("--seed"))
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(
            parse(&args("--seed ten"))
                .unwrap_err()
                .contains("whole number")
        );
        assert!(parse(&args("--simulate 0")).is_err());
        assert!(
            parse(&args("--fly"))
                .unwrap_err()
                .contains("unknown option")
        );
        assert!(parse(&args("--bot --simulate 5")).is_err());
    }
}

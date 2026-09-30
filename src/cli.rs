//! Command-line options.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: smallrogue [--seed N] [--bot] [--no-record]
       smallrogue --replay FILE
       smallrogue --analyze [PATH]
       smallrogue --simulate RUNS [--seed FIRST] [--csv FILE]

  --seed N          play the dungeon for seed N (with --simulate: the
                    first seed of the batch, default 1)
  --bot             let the bot play; press Escape or B to take over
  --no-record       don't save a recording of the run (runs are
                    recorded in ~/.local/share/smallrogue/recordings)
  --replay FILE     watch a recorded run; space pauses, + and - set
                    the speed, any other key stops
  --analyze [PATH]  report on recorded runs: pace, deaths, and how
                    your moves compare with the bot's (PATH: a file or
                    folder, default the recordings folder)
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
    /// Save a recording of each run played in the terminal.
    pub record: bool,
    /// A recording to watch instead of playing.
    pub replay: Option<PathBuf>,
    /// Report on recordings instead of playing: `Some(None)` for the
    /// usual folder.
    pub analyze: Option<Option<PathBuf>>,
}

/// Parses the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        seed: None,
        bot: false,
        simulate: None,
        csv: PathBuf::from("target/sim.csv"),
        record: true,
        replay: None,
        analyze: None,
    };
    let mut args = args.iter().peekable();
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
            "--no-record" => options.record = false,
            "--replay" => options.replay = Some(PathBuf::from(value("--replay")?)),
            "--analyze" => {
                // The path is optional: take the next argument unless
                // it's another option.
                let path = args.next_if(|a| !a.starts_with("--")).map(PathBuf::from);
                options.analyze = Some(path);
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option '{other}'\n\n{USAGE}")),
        }
    }
    if options.bot && options.simulate.is_some() {
        return Err("use either --bot or --simulate, not both".to_string());
    }
    if options.replay.is_some() && (options.bot || options.simulate.is_some()) {
        return Err("--replay can't be combined with --bot or --simulate".to_string());
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

    #[test]
    fn recording_options() {
        let o = parse(&args("--no-record --seed 4")).unwrap();
        assert!(!o.record);
        assert!(
            parse(&args("")).unwrap().record,
            "recording is on by default"
        );
        let o = parse(&args("--replay run.rec")).unwrap();
        assert_eq!(o.replay, Some(PathBuf::from("run.rec")));
        assert!(parse(&args("--replay")).is_err());
        assert!(parse(&args("--replay a.rec --bot")).is_err());
        assert_eq!(parse(&args("--analyze")).unwrap().analyze, Some(None));
        let o = parse(&args("--analyze runs --no-record")).unwrap();
        assert_eq!(o.analyze, Some(Some(PathBuf::from("runs"))));
        let o = parse(&args("--analyze --no-record")).unwrap();
        assert_eq!(o.analyze, Some(None), "an option isn't a path");
    }
}

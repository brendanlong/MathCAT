// Times MathCAT on a corpus file holding one <math> element per line.
// Usage: cargo run --release --example bench_speech -- <corpus> [passes] [speech-output-file]
#![allow(clippy::needless_return)]

use libmathcat::interface::*;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let corpus = std::fs::read_to_string(&args[1]).expect("can't read corpus");
    let passes: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(3);
    let exprs: Vec<&str> = corpus.lines().filter(|l| !l.trim().is_empty()).collect();

    set_rules_dir(libmathcat::abs_rules_dir_path()).unwrap();
    if let Ok(value) = std::env::var("CHECK_RULE_FILES") {
        set_preference("CheckRuleFiles", value).unwrap();
    }
    set_preference("Language", "en").unwrap();
    set_preference("TTS", "none").unwrap();

    let start = Instant::now();
    set_mathml("<math><mi>x</mi></math>").unwrap();
    get_spoken_text().unwrap();
    println!("first call (loads rules): {:.1} ms", start.elapsed().as_secs_f64() * 1e3);

    let mut best = (Duration::from_secs(1 << 40), Duration::ZERO);
    let mut n_errors = 0;
    let mut n_chars = 0;
    let mut all_speech = String::new();
    for pass in 0..passes {
        let (mut t_set, mut t_speak) = (Duration::ZERO, Duration::ZERO);
        n_errors = 0;
        n_chars = 0;
        for expr in &exprs {
            let start = Instant::now();
            let set_result = set_mathml(expr);
            t_set += start.elapsed();
            if set_result.is_err() {
                n_errors += 1;
                continue;
            }
            let start = Instant::now();
            let speech = get_spoken_text();
            t_speak += start.elapsed();
            match speech {
                Ok(s) => {
                    n_chars += s.len();
                    if pass == 0 {
                        all_speech.push_str(&s);
                        all_speech.push('\n');
                    }
                },
                Err(e) => {
                    n_errors += 1;
                    if pass == 0 {
                        all_speech.push_str(&format!("ERROR: {}\n", errors_to_string(&e).lines().next().unwrap_or_default()));
                    }
                },
            }
        }
        if t_set + t_speak < best.0 + best.1 {
            best = (t_set, t_speak);
        }
    }
    if let Some(path) = args.get(3) {
        std::fs::write(path, all_speech).expect("can't write speech output");
    }
    let n = exprs.len() as f64;
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!("{} expressions, {} errors, {} chars of speech", exprs.len(), n_errors, n_chars);
    println!("set_mathml:      {:8.1} ms total, {:7.1} us/expr", ms(best.0), ms(best.0) * 1e3 / n);
    println!("get_spoken_text: {:8.1} ms total, {:7.1} us/expr", ms(best.1), ms(best.1) * 1e3 / n);
    println!("total:           {:8.1} ms total, {:7.1} us/expr", ms(best.0 + best.1), ms(best.0 + best.1) * 1e3 / n);
}

//! Build or check a fixed problem bank for a static page.
use recurrence_lab::{
    bank::{self, Options},
    verifier::Verifier,
};
use std::{fs, path::PathBuf, time::Duration};

fn usage() {
    println!(
        "recurrence-bank generate --seed N --out DIR [--per-level 30] [--jobs N]\n\
         \x20   [--lean-dir PATH] [--lean-timeout SECONDS] [--max-lean-failures N]\n\
         recurrence-bank check --bank DIR\n\n\
         generate: Lv1〜5の問題を選び、有限項の厳密計算とLeanで検証して\n\
         DIR/problems.json と DIR/manifest.json に書き出します。\n\
         check: 公開済みバンクをRecipeから再生成して照合します（Leanは実行しません）。"
    );
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    if command.is_empty() || command == "--help" || command == "-h" {
        usage();
        return Ok(());
    }
    let mut seed = None;
    let mut out = None;
    let mut bank_dir = None;
    let mut lean_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean");
    let mut options = Options {
        seed: 0,
        per_level: 30,
        jobs: std::thread::available_parallelism().map_or(2, |n| n.get()),
        lean_timeout: Duration::from_secs(180),
        max_lean_failures: 5,
    };
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("{arg} の値を指定してください。"))?;
        match arg.as_str() {
            "--seed" => seed = Some(value.parse::<u64>()?),
            "--out" => out = Some(PathBuf::from(value)),
            "--bank" => bank_dir = Some(PathBuf::from(value)),
            "--per-level" => options.per_level = value.parse()?,
            "--jobs" => options.jobs = value.parse()?,
            "--lean-dir" => lean_dir = value.into(),
            "--lean-timeout" => options.lean_timeout = Duration::from_secs(value.parse()?),
            "--max-lean-failures" => options.max_lean_failures = value.parse()?,
            _ => return Err(format!("不明なオプション: {arg}").into()),
        }
    }
    match command.as_str() {
        "generate" => {
            options.seed = seed.ok_or("--seed を指定してください。")?;
            let out = out.ok_or("--out を指定してください。")?;
            let (problems, manifest) = bank::build(&Verifier::new(lean_dir), &options)?;
            fs::create_dir_all(&out)?;
            fs::write(
                out.join("problems.json"),
                serde_json::to_vec_pretty(&problems)?,
            )?;
            fs::write(
                out.join("manifest.json"),
                serde_json::to_string_pretty(&manifest)? + "\n",
            )?;
            println!(
                "Generated {} Lean-verified problems with seed {} in {}",
                manifest["count"],
                options.seed,
                out.display()
            );
        }
        "check" => {
            let dir = bank_dir.ok_or("--bank を指定してください。")?;
            let manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
            println!(
                "{}",
                bank::check(&fs::read(dir.join("problems.json"))?, &manifest)?
            );
        }
        _ => {
            usage();
            return Err(format!("不明なコマンド: {command}").into());
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

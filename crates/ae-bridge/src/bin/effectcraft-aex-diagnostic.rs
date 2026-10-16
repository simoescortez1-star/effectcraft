//! Portable diagnostic CLI; no Python or Rust installation needed on the user's PC.
#[cfg(not(target_arch = "wasm32"))]
fn run() -> Result<(), String> {
    use effectcraft_ae_bridge::diagnostic::{Options, run};
    use std::path::PathBuf;
    let mut options = Options { output: PathBuf::from("effectcraft-aex-diagnostic.json"), ..Default::default() };
    let mut args = std::env::args_os().skip(1);
    let mut count = 0;
    while let Some(arg) = args.next() {
        count += 1;
        if count > 256 {
            return Err("argument limit (256)".into());
        }
        match arg.to_str() {
            Some("--installed") => options.installed = true,
            Some("--execute-matching") => options.execute_matching = true,
            Some("--self-test") => options.self_test = true,
            Some("--folder") => options.folders.push(args.next().ok_or("--folder needs an absolute path")?.into_string().map_err(|_| "folder is not Unicode")?),
            Some("--plugin") => options.plugins.push(PathBuf::from(args.next().ok_or("--plugin needs a path")?)),
            Some("--preset") => options.preset = Some(PathBuf::from(args.next().ok_or("--preset needs a path")?)),
            Some("--out") => options.output = PathBuf::from(args.next().ok_or("--out needs a path")?),
            Some("--match-name") => {
                options.match_name = Some(args.next().ok_or("--match-name needs a value")?.into_string().map_err(|_| "match name is not Unicode")?)
            }
            Some("--help" | "-h") => {
                println!(
                    "EffectCraft AEX local diagnostic\n--installed / --folder ABSOLUTE_DIR: passive inventory only\n--match-name EXACT --execute-matching: execute only exact PiPL effect matches\n--plugin PATH: explicitly execute this AEX (never select it by filename inference)\n--self-test: execute the bundled original Gain plugin at two times\n--preset FILE.ffx: decode using the bundled converter; preserve unsupported data\n--out FILE.json: save local inventory, dependencies, native stages and frame evidence\nExecution requires Windows x64. Logs stay local; commercial plugins are never copied."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option: {} (use --help)", arg.to_string_lossy())),
        }
    }
    run(options)
}
fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = run() {
        eprintln!("AEX diagnostic: {e}");
        std::process::exit(1);
    }
}

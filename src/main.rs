use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use clap::Parser;
use color_eyre::eyre;
use dashmap::DashSet;
use indicatif::{ProgressBar, ProgressStyle};
use parking_lot::Mutex;
use rayon::prelude::*;
use ree_pak_core::utf16_hash::Utf16HashExt;
use ree_path_searcher::{PathSearcher, PathSearcherConfig, SearchResult};
use rustc_hash::FxHashSet;

#[derive(Debug, Parser)]
struct Cli {
    /// Paths to pak files.
    #[arg(short, long)]
    pak: Vec<String>,
    /// A list of paths to pak files. Each line is a path to a pak file.
    #[arg(long)]
    pak_list: Option<String>,
    /// Paths to dmp files.
    #[arg(short, long)]
    dmp: Vec<String>,
    /// Reference path lists. Each line is a reference path to check in input PAKs.
    #[arg(long)]
    ref_list: Vec<String>,
    /// Number of threads to use.
    #[arg(long)]
    threads: Option<usize>,
    /// TOML config for language/prefix/suffix resolving.
    #[arg(long)]
    config: Option<String>,
}

#[derive(Debug)]
struct AppConfig {
    pak: Vec<String>,
    pak_list: Option<String>,
    dmp: Vec<String>,
    ref_list: Vec<String>,
    threads: Option<usize>,
    searcher_config: PathSearcherConfig,
}

fn load_pak_list(pak_list_file: &str) -> eyre::Result<Vec<String>> {
    let mut pak_file_list = vec![];
    let paks = File::open(pak_list_file)?;
    for line in io::BufReader::new(paks).lines() {
        let line = line?;
        if !line.is_empty() && !line.starts_with('#') {
            pak_file_list.push(line);
        }
    }
    Ok(pak_file_list)
}

fn load_ref_list(ref_list_file: &str) -> eyre::Result<Vec<String>> {
    let mut refs = vec![];
    let file = File::open(ref_list_file)?;
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            refs.push(line.to_string());
        }
    }
    Ok(refs)
}

fn canonicalize_ref_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn export_results(result: &SearchResult, extra_full_paths: &[String]) -> eyre::Result<()> {
    let mut raw_writer = std::io::BufWriter::new(File::create("output_raw.list")?);
    let mut writer = std::io::BufWriter::new(File::create("output.list")?);
    let mut written = FxHashSet::default();

    for (raw_path, indexes) in &result.found_paths {
        for index in indexes {
            writeln!(writer, "{}", index.full_path)?;
            written.insert(index.full_path.hash_mixed());
        }
        writeln!(raw_writer, "{}", raw_path)?;
    }

    for path in extra_full_paths {
        let hash = path.hash_mixed();
        if written.insert(hash) {
            writeln!(writer, "{path}")?;
        }
    }

    let mut unknown_writer = std::io::BufWriter::new(File::create("unknown.list")?);
    for path in &result.unknown_paths {
        writeln!(unknown_writer, "{}", path)?;
    }

    Ok(())
}

fn run(app: AppConfig) -> eyre::Result<()> {
    if app.pak.is_empty() && app.pak_list.is_none() && app.dmp.is_empty() && app.ref_list.is_empty()
    {
        eprintln!(
            "Error: No PAK/DMP/reference list specified. Use --pak, --pak-list, --dmp, or --ref-list."
        );
        std::process::exit(1);
    }

    if !app.ref_list.is_empty() && app.pak.is_empty() && app.pak_list.is_none() {
        eprintln!("Error: --ref-list requires input PAKs. Use --pak or --pak-list.");
        std::process::exit(1);
    }

    // set rayon threads
    let threads = if let Some(threads) = app.threads {
        threads.min(num_cpus::get())
    } else {
        num_cpus::get().min(8)
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()?;

    let start = Instant::now();

    let mut builder = PathSearcher::builder();

    builder = builder.with_config(app.searcher_config);

    if !app.pak.is_empty() {
        builder = builder.with_pak_paths(&app.pak);
    }

    if let Some(pak_list) = &app.pak_list {
        let paths = load_pak_list(pak_list)?;
        builder = builder.with_pak_paths(&paths);
    }

    let searcher = builder.build()?;

    if !app.pak.is_empty() || app.pak_list.is_some() {
        println!("Input pak total file count: {}", searcher.pak_file_count());
    }

    let mut all_results = ree_path_searcher::SearchResult {
        found_paths: vec![],
        unknown_paths: rustc_hash::FxHashSet::default(),
    };

    if !app.dmp.is_empty() {
        for dmp in &app.dmp {
            eprintln!("Scanning {dmp}..");
            let progress_bar = ProgressBar::new(100);
            progress_bar.enable_steady_tick(Duration::from_millis(100));
            progress_bar.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {per_sec} {msg}",
                    )
                    .unwrap()
                    .progress_chars("##-"),
            );

            let result =
                searcher.search_memory_dump_with_progress(dmp, |current: u64, total: u64| {
                    progress_bar.set_length(total);
                    progress_bar.set_position(current);
                })?;

            progress_bar.finish_with_message("Scan dump finished.");
            all_results.found_paths.extend(result.found_paths);
            all_results.unknown_paths.extend(result.unknown_paths);
        }
    }

    if searcher.pak_file_count() != 0 {
        eprintln!("Scanning all PAK files..");
        let progress_bar = ProgressBar::new(searcher.pak_file_count() as u64);
        progress_bar.enable_steady_tick(Duration::from_millis(100));
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {per_sec} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );

        let result = searcher.search_pak_files_with_progress(|current: u64, total: u64| {
            progress_bar.set_length(total);
            progress_bar.set_position(current);
        })?;

        progress_bar.finish_with_message("Scan pak files finished.");
        all_results.found_paths.extend(result.found_paths);
        all_results.unknown_paths.extend(result.unknown_paths);
    }

    // resolve reference list if provided
    let mut ref_matched_full_paths: Vec<String> = vec![];
    if !app.ref_list.is_empty() {
        if searcher.pak_collection().is_some() {
            let mut refs: Vec<String> = vec![];
            for file in &app.ref_list {
                refs.extend(load_ref_list(file)?);
            }

            eprintln!("Resolving reference list..");
            let progress_bar = ProgressBar::new(refs.len() as u64);
            progress_bar.enable_steady_tick(Duration::from_millis(100));
            progress_bar.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {per_sec} {msg}")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let matched: DashSet<String> = DashSet::default();
            let missing_count = AtomicUsize::new(0);
            let error_count = AtomicUsize::new(0);
            let first_errors: Mutex<Vec<(String, String)>> = Mutex::new(vec![]);

            let pb = progress_bar.clone();
            refs.par_iter().for_each(|r| {
                match searcher.resolve_reference_line(r) {
                    Ok(infos) if !infos.is_empty() => {
                        for info in infos {
                            matched.insert(canonicalize_ref_path(&info.full_path));
                        }
                    }
                    Ok(_) => {
                        missing_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(err) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                        let mut guard = first_errors.lock();
                        if guard.len() < 5 {
                            guard.push((r.clone(), format!("{err:#}")));
                        }
                    }
                }
                pb.inc(1);
            });
            progress_bar.finish_with_message("Resolve reference list finished.");

            ref_matched_full_paths = matched.into_iter().collect();
            ref_matched_full_paths.sort_unstable();
            eprintln!(
                "Reference list: matched {} paths, missing {}, errors {}.",
                ref_matched_full_paths.len(),
                missing_count.load(Ordering::Relaxed),
                error_count.load(Ordering::Relaxed)
            );
            let first_errors = first_errors.into_inner();
            if !first_errors.is_empty() {
                eprintln!("Reference list: first {} errors:", first_errors.len());
                for (line, err) in first_errors {
                    eprintln!("  - {line}: {err}");
                }
            }
        } else {
            eprintln!(
                "Warning: --ref-list provided but no PAK files loaded; skipping reference checks."
            );
        }
    }

    println!("Sorting results..");
    all_results
        .found_paths
        .sort_unstable_by(|(p, _), (q, _)| p.cmp(q));
    all_results.found_paths.dedup_by(|(p, _), (q, _)| p == q);

    println!("Exporting results..");
    export_results(&all_results, &ref_matched_full_paths)?;

    let elapsed = start.elapsed();
    println!("Elapsed: {:.2?} seconds", elapsed.as_secs_f32());

    Ok(())
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let searcher_config = if let Some(path) = &cli.config {
        println!("Loading config from {}", path);
        PathSearcherConfig::from_toml_file(path)?
    } else {
        let default_path = Path::new("config.toml");
        if default_path.exists() {
            println!("Loading config from {}", default_path.display());
            PathSearcherConfig::from_toml_file(default_path)?
        } else {
            println!("Using built-in config");
            PathSearcherConfig::default()
        }
    };

    run(AppConfig {
        pak: cli.pak,
        pak_list: cli.pak_list,
        dmp: cli.dmp,
        ref_list: cli.ref_list,
        threads: cli.threads,
        searcher_config,
    })
}

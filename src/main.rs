use std::{
    fs::File,
    io::{self, BufRead},
    time::Instant,
};

use clap::Parser;
use color_eyre::eyre;
use ree_path_searcher::{export_results, PathSearcher};

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

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let cli = Cli::parse();

    if cli.pak.is_empty() && cli.pak_list.is_none() && cli.dmp.is_empty() {
        eprintln!("Error: No PAK or DMP files specified. Use --pak, --pak-list, or --dmp options.");
        std::process::exit(1);
    }

    let start = Instant::now();

    let mut builder = PathSearcher::builder();

    if !cli.pak.is_empty() {
        builder = builder.with_pak_files(&cli.pak);
    }

    if let Some(pak_list) = &cli.pak_list {
        let paths = load_pak_list(pak_list)?;
        builder = builder.with_pak_files(&paths);
    }

    let searcher = builder.build()?;

    if !cli.pak.is_empty() || cli.pak_list.is_some() {
        println!("Input pak total file count: {}", searcher.pak_file_count());
    }

    let mut all_results = ree_path_searcher::SearchResult {
        found_paths: vec![],
        unknown_paths: rustc_hash::FxHashSet::default(),
    };

    if !cli.dmp.is_empty() {
        for dmp in &cli.dmp {
            eprintln!("Scanning {dmp}..");
            let result = searcher.search_memory_dump(dmp)?;
            all_results.found_paths.extend(result.found_paths);
            all_results.unknown_paths.extend(result.unknown_paths);
        }
    }

    if !cli.pak.is_empty() {
        eprintln!("Scanning all PAK files..");
        let result = searcher.search_pak_files()?;
        all_results.found_paths.extend(result.found_paths);
        all_results.unknown_paths.extend(result.unknown_paths);
    }

    println!("Sorting results..");
    all_results.found_paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    all_results.found_paths.dedup_by(|(p, _), (q, _)| p == q);

    println!("Exporting results..");
    export_results(&all_results)?;

    let elapsed = start.elapsed();
    println!("Elapsed: {:.2?} seconds", elapsed.as_secs_f32());

    Ok(())
}

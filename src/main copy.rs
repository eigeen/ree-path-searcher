mod pak;
mod suffix;
mod utils;

use std::io::Write;
use std::{fs::File, time::Duration};

use color_eyre::eyre::{self, Context};
use indicatif::{ProgressBar, ProgressStyle};
use minidump::{Minidump, MinidumpMemory64List};
use parking_lot::Mutex;
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelExtend, ParallelIterator,
};
use ree_pak_core::{filename::FileNameExt, write};
use rustc_hash::{FxHashMap, FxHashSet};
use suffix::I18nPakFileInfo;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let start = std::time::Instant::now();
    // // search_path_original_bench(
    // search_path_optimized_bench(
    //     vec![
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak".to_string(),
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.patch_001.pak".to_string(),
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.patch_002.pak".to_string(),
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak".to_string(),
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak.patch_001.pak".to_string(),
    //         "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak.patch_002.pak".to_string(),
    //     ],
    // )?;

    search_path_optimized(
        vec![
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak".to_string(),
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.patch_001.pak".to_string(),
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.patch_002.pak".to_string(),
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak".to_string(),
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak.patch_001.pak".to_string(),
            "E:\\SteamLibrary\\steamapps\\common\\MonsterHunterWilds\\re_chunk_000.pak.sub_000.pak.patch_002.pak".to_string(),
        ],
        vec!["E:\\MonsterHunterWilds_full.dmp".to_string()],
        // vec![],
    )?;
    let elapsed = start.elapsed();
    println!("Elapsed: {:.2?} seconds", elapsed.as_secs_f32());

    Ok(())
}

fn accept_char(c: u8) -> bool {
    if c == b' ' {
        return true;
    }
    if !c.is_ascii_graphic() {
        return false;
    }
    #[allow(clippy::needless_raw_string_hashes)]
    if br###""*\:<>?*|"###.contains(&c) {
        return false;
    }
    true
}

fn validate_path(path: &str) -> bool {
    let Some((_, tail)) = path.rsplit_once('/') else {
        return false;
    };
    // has extension
    let Some(dot_pos) = tail.find('.') else {
        return false;
    };
    // non-empty extension
    if !(dot_pos != 0 && dot_pos != tail.len() - 1) {
        return false;
    }
    true
}

fn search_path_optimized_bench(pak: Vec<String>) -> eyre::Result<()> {
    let pak = pak::PakCollection::from_paths(&pak)?;
    let counter = std::sync::atomic::AtomicU32::new(0);

    let mut paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];
    // let mut file_versions = FxHashMap::default();

    let search_memory = |memory: &[u8]| {
        let mut paths = vec![];
        // path is utf-16
        // find slashes '/'
        const SLASH_U16: [u8; 2] = [b'/', 0];
        let mut pos = 0;
        while let Some(mut slash_pos) = memchr::memmem::find(&memory[pos..], &SLASH_U16) {
            slash_pos += pos; // fix offset
            pos = (slash_pos + 2).min(memory.len());
            // locate the start of the path
            let mut begin = slash_pos;
            loop {
                if begin < 2 {
                    break;
                }
                let earlier = begin - 2;
                if !accept_char(memory[earlier]) {
                    break;
                }
                if memory[earlier + 1] != 0 {
                    break;
                }
                begin = earlier;
            }
            if begin == slash_pos {
                // bad path
                continue;
            }
            // locate the end of the path
            let mut end = slash_pos + 2;
            loop {
                if end >= memory.len() - 1 {
                    break;
                }
                let later = end;
                if !accept_char(memory[later]) {
                    break;
                }
                if memory[later + 1] != 0 {
                    break;
                }
                end = later + 2;
            }
            if end == slash_pos {
                // bad path
                continue;
            }
            pos = (end + 2).min(memory.len());
            // decode path
            let Some(path) = utils::string_from_utf16_bytes(&memory[begin..end]) else {
                continue;
            };
            if validate_path(&path) {
                let Ok(file_hashes) = suffix::find_path_i18n(&pak, &path) else {
                    println!("Warning: failed to find file {path}");
                    continue;
                };
                paths.push((path, file_hashes));
            }
        }

        let counter_prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if counter_prev % 100 == 0 {
            eprintln!("Found {counter_prev} paths so far")
        }

        Ok(paths)
    };

    let indexes = pak
        .path_hashes
        .iter()
        .take(1000)
        .collect::<FxHashMap<_, _>>();
    eprintln!("Scanning all PAK files..");
    paths.par_extend(
        indexes
            .keys()
            .par_bridge()
            .map(|&hash| {
                let file = pak.read_file_by_hash(*hash)?;
                search_memory(&file)
            })
            .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
    );

    paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    paths.dedup_by(|(p, _), (q, _)| p == q);

    for (path, index) in paths {
        println!("{path} $ {index:?}");
    }

    Ok(())
}

fn search_path_optimized(pak: Vec<String>, dmp: Vec<String>) -> eyre::Result<()> {
    let pak = pak::PakCollection::from_paths(&pak)?;
    println!("Input pak total file count: {}", pak.path_hashes.len());
    let bar = ProgressBar::new_spinner().with_style(
        ProgressStyle::default_spinner().template("{spinner} [Dump] Paths found: {pos} {msg}")?,
    );
    bar.enable_steady_tick(Duration::from_millis(100));

    let mut all_paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];
    let unk_paths = Mutex::new(FxHashSet::default());

    let search_memory = |memory: &[u8]| {
        let mut paths = vec![];
        // path is utf-16
        // find slashes '/'
        const SLASH_U16: [u8; 2] = [b'/', 0];
        let mut pos = 0;
        while let Some(mut slash_pos) = memchr::memmem::find(&memory[pos..], &SLASH_U16) {
            slash_pos += pos; // fix offset
            pos = (slash_pos + 2).min(memory.len());
            // locate the start of the path
            let mut begin = slash_pos;
            loop {
                if begin < 2 {
                    break;
                }
                let earlier = begin - 2;
                if !accept_char(memory[earlier]) {
                    break;
                }
                if memory[earlier + 1] != 0 {
                    break;
                }
                begin = earlier;
            }
            if begin == slash_pos {
                // bad path
                continue;
            }
            // locate the end of the path
            let mut end = slash_pos + 2;
            loop {
                if end >= memory.len() - 1 {
                    break;
                }
                let later = end;
                if !accept_char(memory[later]) {
                    break;
                }
                if memory[later + 1] != 0 {
                    break;
                }
                end = later + 2;
            }
            if end == slash_pos {
                // bad path
                continue;
            }
            pos = (end + 2).min(memory.len());
            // decode path
            let Some(path) = utils::string_from_utf16_bytes(&memory[begin..end]) else {
                continue;
            };
            if validate_path(&path) {
                let Ok(file_hashes) = suffix::find_path_i18n(&pak, &path) else {
                    // println!("Warning: failed to find file {path}");
                    unk_paths.lock().insert(path);
                    continue;
                };
                paths.push((path, file_hashes));
                // paths.push((path, vec![]));
            }
        }

        bar.inc(1);
        Ok(paths)
    };

    for dmp in dmp {
        eprintln!("Scanning {dmp}..");

        let dmp = Minidump::read_path(dmp)?;
        let memory = dmp
            .get_stream::<MinidumpMemory64List>()
            .context("No full dump memory found")?;

        let mut memory: Vec<_> = memory.iter().collect();
        // merge memory blocks
        memory.sort_by_key(|memory| memory.base_address);
        use std::borrow::*;
        struct Block<'a> {
            base: u64,
            len: u64,
            data: Cow<'a, [u8]>,
        }

        let mut memory_blocks: Vec<Block> = vec![];
        for piece in memory {
            if let Some(prev) = memory_blocks.last_mut() {
                if prev.base + prev.len == piece.base_address {
                    prev.data.to_mut().extend(piece.bytes);
                    prev.len += piece.size;
                    continue;
                }
            }
            memory_blocks.push(Block {
                base: piece.base_address,
                len: piece.size,
                data: Cow::Borrowed(piece.bytes),
            })
        }

        all_paths.par_extend(
            memory_blocks
                .par_iter()
                .map(|memory| search_memory(&memory.data))
                .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
        );
    }

    bar.set_position(0);
    bar.set_style(
        ProgressStyle::default_spinner().template("{spinner} [Pak] Paths found: {pos} {msg}")?,
    );
    let indexes = pak.path_hashes.clone();
    // // DEBUG
    // let hash = ree_pak_core::filename::FileNameFull::from(
    //     "natives/STM/GameDesign/Catalog/00_00/Data/EnemyPackageList.user.3",
    // )
    // .hash_mixed();
    // let indexes = indexes
    //     .into_par_iter()
    //     .filter(|(key, _)| *key == hash)
    //     .collect::<FxHashMap<_, _>>();
    // println!("indexes: {}", indexes.len());
    eprintln!("Scanning all PAK files..");
    all_paths.par_extend(
        indexes
            .keys()
            .par_bridge()
            .map(|hash| {
                let file = pak.read_file_by_hash(*hash)?;
                search_memory(&file)
            })
            .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
    );

    println!("Sorting results..");
    all_paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    all_paths.dedup_by(|(p, _), (q, _)| p == q);

    // for (path, index) in paths {
    //     println!("{path} $ {index:?}");
    // }
    println!("Exporting results..");
    let file = File::create("output_raw.list")?;
    let mut raw_writer = std::io::BufWriter::new(file);
    let file = File::create("output.list")?;
    let mut writer = std::io::BufWriter::new(file);
    for (raw_path, indexes) in all_paths {
        for index in indexes {
            writeln!(writer, "{}", index.full_path)?;
        }
        writeln!(raw_writer, "{}", raw_path)?;
    }
    let file = File::create("unknown.list")?;
    let mut writer = std::io::BufWriter::new(file);
    for path in unk_paths.lock().iter() {
        writeln!(writer, "{}", path)?;
    }

    Ok(())
}

fn search_path_optimized_old(pak: Vec<String>, dmp: Vec<String>) -> eyre::Result<()> {
    let pak = pak::PakCollection::from_paths(&pak)?;
    let counter = std::sync::atomic::AtomicU32::new(0);

    let mut paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];

    fn accept_char(c: u8) -> bool {
        if c == b' ' {
            return true;
        }
        if !c.is_ascii_graphic() {
            return false;
        }
        #[allow(clippy::needless_raw_string_hashes)]
        if br###""*\:<>?*|"###.contains(&c) {
            return false;
        }
        true
    }

    let search_memory = |memory: &[u8]| {
        let mut paths = vec![];
        for &suffix in suffix::SUFFIX_MAP_FULL.keys() {
            let mut full_suffix = vec![0; (suffix.len() + 2) * 2];
            full_suffix[0] = b'.';
            for (i, &c) in suffix.as_bytes().iter().enumerate() {
                full_suffix[i * 2 + 2] = c;
            }
            let mut pos = 0;
            while let Some(mut suffix_pos) = memchr::memmem::find(&memory[pos..], &full_suffix) {
                suffix_pos += pos; // fix offset
                pos = (suffix_pos + full_suffix.len()).min(memory.len());

                let end = suffix_pos + full_suffix.len() - 2;
                let mut begin = suffix_pos;
                loop {
                    if begin < 2 {
                        break;
                    }
                    let earlier = begin - 2;
                    if !accept_char(memory[earlier]) {
                        break;
                    }
                    if memory[earlier + 1] != 0 {
                        break;
                    }

                    begin = earlier;
                }
                let mut path = String::new();
                for pos in (begin..end).step_by(2) {
                    path.push(char::from(memory[pos]));
                }
                // let index = pak.lock().unwrap().find_file_i18n(&path)?;
                let file_hashes = suffix::find_path_i18n(&pak, &path)?;
                paths.push((path, file_hashes));
            }
        }

        let counter_prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if counter_prev % 100 == 0 {
            eprintln!("Found {counter_prev} paths so far")
        }

        Ok(paths)
    };

    for dmp in dmp {
        eprintln!("Scanning {dmp}..");

        let dmp = Minidump::read_path(dmp)?;
        let memory = dmp
            .get_stream::<MinidumpMemory64List>()
            .context("No full dump memory found")?;

        let mut memory: Vec<_> = memory.iter().collect();
        // merge memory blocks
        memory.sort_by_key(|memory| memory.base_address);
        use std::borrow::*;
        struct Block<'a> {
            base: u64,
            len: u64,
            data: Cow<'a, [u8]>,
        }

        let mut memory_blocks: Vec<Block> = vec![];
        for piece in memory {
            if let Some(prev) = memory_blocks.last_mut() {
                if prev.base + prev.len == piece.base_address {
                    prev.data.to_mut().extend(piece.bytes);
                    prev.len += piece.size;
                    continue;
                }
            }
            memory_blocks.push(Block {
                base: piece.base_address,
                len: piece.size,
                data: Cow::Borrowed(piece.bytes),
            })
        }

        paths.par_extend(
            memory_blocks
                .par_iter()
                .map(|memory| search_memory(&memory.data))
                .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
        );
    }

    let indexes = pak.path_hashes.clone();
    eprintln!("Scanning all PAK files..");
    paths.par_extend(
        indexes
            .keys()
            .par_bridge()
            .map(|hash| {
                let file = pak.read_file_by_hash(*hash)?;
                search_memory(&file)
            })
            .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
    );

    paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    paths.dedup_by(|(p, _), (q, _)| p == q);

    // for (path, index) in paths {
    //     println!("{path} $ {index:?}");
    // }
    let file = File::create("output.list")?;
    let mut writer = std::io::BufWriter::new(file);
    for (_, indexes) in paths {
        for index in indexes {
            writeln!(writer, "{}", index.full_path)?;
        }
    }

    Ok(())
}

fn search_path_original_bench(pak: Vec<String>) -> eyre::Result<()> {
    let pak = pak::PakCollection::from_paths(&pak)?;
    let counter = std::sync::atomic::AtomicU32::new(0);

    let mut paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];

    fn accept_char(c: u8) -> bool {
        if c == b' ' {
            return true;
        }
        if !c.is_ascii_graphic() {
            return false;
        }
        #[allow(clippy::needless_raw_string_hashes)]
        if br###""*\:<>?*|"###.contains(&c) {
            return false;
        }
        true
    }

    let search_memory = |memory: &[u8]| {
        let mut paths = vec![];
        for &suffix in suffix::SUFFIX_MAP_FULL.keys() {
            let mut full_suffix = vec![0; (suffix.len() + 2) * 2];
            full_suffix[0] = b'.';
            for (i, &c) in suffix.as_bytes().iter().enumerate() {
                full_suffix[i * 2 + 2] = c;
            }
            for (suffix_pos, window) in memory.windows(full_suffix.len()).enumerate() {
                if window != full_suffix {
                    continue;
                }
                let end = suffix_pos + full_suffix.len() - 2;
                let mut begin = suffix_pos;
                loop {
                    if begin < 2 {
                        break;
                    }
                    let earlier = begin - 2;
                    if !accept_char(memory[earlier]) {
                        break;
                    }
                    if memory[earlier + 1] != 0 {
                        break;
                    }

                    begin = earlier;
                }
                let mut path = String::new();
                for pos in (begin..end).step_by(2) {
                    path.push(char::from(memory[pos]));
                }
                // let index = pak.lock().unwrap().find_file_i18n(&path)?;
                let file_hashes = suffix::find_path_i18n(&pak, &path)?;
                paths.push((path, file_hashes));
            }
        }

        let counter_prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if counter_prev % 100 == 0 {
            eprintln!("Found {counter_prev} paths so far")
        }

        Ok(paths)
    };

    let indexes = pak
        .path_hashes
        .iter()
        .take(1000)
        .collect::<FxHashMap<_, _>>();
    eprintln!("Scanning all PAK files..");
    paths.par_extend(
        indexes
            .keys()
            .par_bridge()
            .map(|&hash| {
                let file = pak.read_file_by_hash(*hash)?;
                search_memory(&file)
            })
            .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
    );

    paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    paths.dedup_by(|(p, _), (q, _)| p == q);

    for (path, index) in paths {
        println!("{path} $ {index:?}");
    }

    Ok(())
}

fn search_path(pak: Vec<String>, dmp: Vec<String>) -> eyre::Result<()> {
    let pak = pak::PakCollection::from_paths(&pak)?;
    let counter = std::sync::atomic::AtomicU32::new(0);

    let mut paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];

    fn accept_char(c: u8) -> bool {
        if c == b' ' {
            return true;
        }
        if !c.is_ascii_graphic() {
            return false;
        }
        #[allow(clippy::needless_raw_string_hashes)]
        if br###""*\:<>?*|"###.contains(&c) {
            return false;
        }
        true
    }

    let search_memory = |memory: &[u8]| {
        let mut paths = vec![];
        for &suffix in suffix::SUFFIX_MAP_FULL.keys() {
            let mut full_suffix = vec![0; (suffix.len() + 2) * 2];
            full_suffix[0] = b'.';
            for (i, &c) in suffix.as_bytes().iter().enumerate() {
                full_suffix[i * 2 + 2] = c;
            }
            for (suffix_pos, window) in memory.windows(full_suffix.len()).enumerate() {
                if window != full_suffix {
                    continue;
                }
                let end = suffix_pos + full_suffix.len() - 2;
                let mut begin = suffix_pos;
                loop {
                    if begin < 2 {
                        break;
                    }
                    let earlier = begin - 2;
                    if !accept_char(memory[earlier]) {
                        break;
                    }
                    if memory[earlier + 1] != 0 {
                        break;
                    }

                    begin = earlier;
                }
                let mut path = String::new();
                for pos in (begin..end).step_by(2) {
                    path.push(char::from(memory[pos]));
                }
                // let index = pak.lock().unwrap().find_file_i18n(&path)?;
                let file_hashes = suffix::find_path_i18n(&pak, &path)?;
                paths.push((path, file_hashes));
            }
        }

        let counter_prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if counter_prev % 100 == 0 {
            eprintln!("Found {counter_prev} paths so far")
        }

        Ok(paths)
    };

    for dmp in dmp {
        eprintln!("Scanning {dmp}..");

        let dmp = Minidump::read_path(dmp)?;
        let memory = dmp
            .get_stream::<MinidumpMemory64List>()
            .context("No full dump memory found")?;

        let mut memory: Vec<_> = memory.iter().collect();
        // merge memory blocks
        memory.sort_by_key(|memory| memory.base_address);
        use std::borrow::*;
        struct Block<'a> {
            base: u64,
            len: u64,
            data: Cow<'a, [u8]>,
        }

        let mut memory_blocks: Vec<Block> = vec![];
        for piece in memory {
            if let Some(prev) = memory_blocks.last_mut() {
                if prev.base + prev.len == piece.base_address {
                    prev.data.to_mut().extend(piece.bytes);
                    prev.len += piece.size;
                    continue;
                }
            }
            memory_blocks.push(Block {
                base: piece.base_address,
                len: piece.size,
                data: Cow::Borrowed(piece.bytes),
            })
        }

        paths.par_extend(
            memory_blocks
                .par_iter()
                .map(|memory| search_memory(&memory.data))
                .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
        );
    }
    for (path, index) in paths {
        println!("{path} $ {index:?}");
    }
    return Ok(());

    let indexes = pak.path_hashes.clone();
    eprintln!("Scanning all PAK files..");
    paths.par_extend(
        indexes
            .keys()
            .par_bridge()
            .map(|hash| {
                let file = pak.read_file_by_hash(*hash)?;
                search_memory(&file)
            })
            .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
    );

    paths.sort_by(|(p, _), (q, _)| p.cmp(q));
    paths.dedup_by(|(p, _), (q, _)| p == q);

    for (path, index) in paths {
        println!("{path} $ {index:?}");
    }

    Ok(())
}

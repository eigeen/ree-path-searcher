pub mod pak;
pub mod suffix;
pub mod utils;

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use color_eyre::eyre::{self, Context};
use minidump::{Minidump, MinidumpMemory64List};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelExtend, ParallelIterator};
use rustc_hash::FxHashSet;
use suffix::I18nPakFileInfo;

pub use pak::PakCollection;
pub use suffix::{I18nPakFileInfo as PathInfo, find_path_i18n};
pub use utils::string_from_utf16_bytes;

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub found_paths: Vec<(String, Vec<I18nPakFileInfo>)>,
    pub unknown_paths: FxHashSet<String>,
}

#[derive(Default)]
pub struct PathSearcherBuilder {
    pak_paths: Vec<PathBuf>,
}

impl PathSearcherBuilder {
    pub fn with_pak_file(mut self, pak_path: impl AsRef<Path>) -> eyre::Result<Self> {
        self.pak_paths.push(pak_path.as_ref().to_path_buf());
        Ok(self)
    }

    pub fn with_pak_files(mut self, pak_paths: &[impl AsRef<Path>]) -> Self {
        self.pak_paths
            .extend(pak_paths.iter().map(|p| p.as_ref().to_path_buf()));
        self
    }

    pub fn build(self) -> eyre::Result<PathSearcher> {
        if self.pak_paths.is_empty() {
            return Ok(PathSearcher {
                pak_collection: None,
            });
        }

        let pak_collection = PakCollection::from_paths(&self.pak_paths)?;
        Ok(PathSearcher {
            pak_collection: Some(pak_collection),
        })
    }
}

pub struct PathSearcher {
    pak_collection: Option<PakCollection<'static, io::BufReader<File>>>,
}

impl PathSearcher {
    pub fn new() -> Self {
        Self {
            pak_collection: None,
        }
    }

    pub fn builder() -> PathSearcherBuilder {
        PathSearcherBuilder::default()
    }

    pub fn pak_collection(&self) -> Option<&PakCollection<'static, io::BufReader<File>>> {
        self.pak_collection.as_ref()
    }

    pub fn pak_file_count(&self) -> usize {
        self.pak_collection
            .as_ref()
            .map(|c| c.path_hashes.len())
            .unwrap_or(0)
    }

    pub fn search_memory_dump(&self, dmp_path: &str) -> eyre::Result<SearchResult> {
        let mut all_paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];
        let unk_paths = Mutex::new(FxHashSet::default());

        let dmp = Minidump::read_path(dmp_path)?;
        let memory = dmp
            .get_stream::<MinidumpMemory64List>()
            .context("No full dump memory found")?;

        let mut memory: Vec<_> = memory.iter().collect();
        memory.sort_by_key(|memory| memory.base_address);

        struct Block<'a> {
            base: u64,
            len: u64,
            data: Cow<'a, [u8]>,
        }

        let mut memory_blocks: Vec<Block> = vec![];
        for piece in memory {
            if let Some(prev) = memory_blocks.last_mut()
                && prev.base + prev.len == piece.base_address
            {
                prev.data.to_mut().extend(piece.bytes);
                prev.len += piece.size;
                continue;
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
                .map(|memory| self.search_memory(&memory.data, &unk_paths))
                .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
        );

        all_paths.sort_by(|(p, _), (q, _)| p.cmp(q));
        all_paths.dedup_by(|(p, _), (q, _)| p == q);

        Ok(SearchResult {
            found_paths: all_paths,
            unknown_paths: unk_paths.into_inner(),
        })
    }

    pub fn search_pak_files(&self) -> eyre::Result<SearchResult> {
        let Some(pak_collection) = &self.pak_collection else {
            return Ok(SearchResult::default());
        };

        let mut all_paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];
        let unk_paths = Mutex::new(FxHashSet::default());

        let indexes = pak_collection.path_hashes.clone();
        all_paths.par_extend(
            indexes
                .keys()
                .par_bridge()
                .map(|hash| {
                    let file = pak_collection.read_file_by_hash(*hash)?;
                    self.search_memory(&file, &unk_paths)
                })
                .flat_map_iter(|paths: eyre::Result<_>| paths.unwrap()),
        );

        all_paths.sort_by(|(p, _), (q, _)| p.cmp(q));
        all_paths.dedup_by(|(p, _), (q, _)| p == q);

        Ok(SearchResult {
            found_paths: all_paths,
            unknown_paths: unk_paths.into_inner(),
        })
    }

    fn search_memory(
        &self,
        memory: &[u8],
        unk_paths: &Mutex<FxHashSet<String>>,
    ) -> eyre::Result<Vec<(String, Vec<I18nPakFileInfo>)>> {
        let mut paths = vec![];
        const SLASH_U16: [u8; 2] = [b'/', 0];
        let mut pos = 0;

        while let Some(mut slash_pos) = memchr::memmem::find(&memory[pos..], &SLASH_U16) {
            slash_pos += pos;
            pos = (slash_pos + 2).min(memory.len());

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
                continue;
            }

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
                continue;
            }
            pos = (end + 2).min(memory.len());

            let Some(path) = utils::string_from_utf16_bytes(&memory[begin..end]) else {
                continue;
            };

            if validate_path(&path) {
                if let Some(pak) = &self.pak_collection {
                    let Ok(file_hashes) = suffix::find_path_i18n(pak, &path) else {
                        unk_paths.lock().insert(path);
                        continue;
                    };
                    paths.push((path, file_hashes));
                } else {
                    paths.push((path, vec![]));
                }
            }
        }

        Ok(paths)
    }
}

impl Default for PathSearcher {
    fn default() -> Self {
        Self::new()
    }
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
    let Some(dot_pos) = tail.find('.') else {
        return false;
    };
    !(dot_pos == 0 || dot_pos == tail.len() - 1)
}

pub fn export_results(result: &SearchResult) -> eyre::Result<()> {
    let file = File::create("output_raw.list")?;
    let mut raw_writer = std::io::BufWriter::new(file);
    let file = File::create("output.list")?;
    let mut writer = std::io::BufWriter::new(file);

    for (raw_path, indexes) in &result.found_paths {
        for index in indexes {
            writeln!(writer, "{}", index.full_path)?;
        }
        writeln!(raw_writer, "{}", raw_path)?;
    }

    let file = File::create("unknown.list")?;
    let mut writer = std::io::BufWriter::new(file);
    for path in &result.unknown_paths {
        writeln!(writer, "{}", path)?;
    }

    Ok(())
}

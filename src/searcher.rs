mod filter;
mod suffix;

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use color_eyre::eyre::{self, Context};
use dashmap::DashMap;
use minidump::{Minidump, MinidumpMemory64List};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelExtend, ParallelIterator};
use rustc_hash::{FxBuildHasher, FxHashSet};
use suffix::I18nPakFileInfo;

use crate::pak::PakCollection;
use crate::searcher::filter::{DefaultFilter, FileContext, Filter};
use crate::utils;

pub trait ProgressCallback {
    fn on_progress(&self, current: u64, total: u64);
}

impl<F> ProgressCallback for F
where
    F: for<'a> Fn(u64, u64),
{
    fn on_progress(&self, current: u64, total: u64) {
        self(current, total);
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// Found paths in PAK files.
    /// (path, detailed infos)
    pub found_paths: Vec<(String, Vec<I18nPakFileInfo>)>,
    pub unknown_paths: FxHashSet<String>,
}

pub struct PathSearcherBuilder<R> {
    pak_source: Vec<R>,
    filter: Option<Box<dyn Filter + Send + Sync>>,
}

impl<R> Default for PathSearcherBuilder<R> {
    fn default() -> Self {
        Self {
            pak_source: vec![],
            filter: Some(Box::new(DefaultFilter)),
        }
    }
}

impl<R: Read + Seek + Send> PathSearcherBuilder<R> {
    pub fn with_pak_file(mut self, reader: R) -> eyre::Result<Self> {
        self.pak_source.push(reader);
        Ok(self)
    }

    pub fn with_pak_files(mut self, readers: impl IntoIterator<Item = R>) -> Self {
        self.pak_source.extend(readers);
        self
    }

    pub fn with_filter(mut self, filter: Option<Box<dyn Filter + Send + Sync>>) -> Self {
        self.filter = filter;
        self
    }

    pub fn build(self) -> eyre::Result<PathSearcher<R>> {
        let pak_collection = if self.pak_source.is_empty() {
            None
        } else {
            Some(PakCollection::from_readers(self.pak_source)?)
        };

        Ok(PathSearcher {
            pak_collection,
            path_cache: DashMap::default(),
            filter: self.filter,
        })
    }
}

impl PathSearcherBuilder<io::BufReader<File>> {
    pub fn with_pak_paths(self, paths: &[impl AsRef<Path>]) -> Self {
        self.with_pak_files(
            paths
                .iter()
                .map(|p| io::BufReader::new(File::open(p).unwrap()))
                .collect::<Vec<_>>(),
        )
    }
}

pub struct PathSearcher<R> {
    pak_collection: Option<PakCollection<'static, R>>,
    path_cache: DashMap<String, Option<Vec<I18nPakFileInfo>>, FxBuildHasher>,
    filter: Option<Box<dyn Filter + Send + Sync>>,
}

impl<R> Default for PathSearcher<R> {
    fn default() -> Self {
        Self {
            pak_collection: None,
            path_cache: DashMap::default(),
            filter: None,
        }
    }
}

impl<R> PathSearcher<R> {
    pub fn builder() -> PathSearcherBuilder<R> {
        PathSearcherBuilder::default()
    }

    pub fn pak_collection(&self) -> Option<&PakCollection<'static, R>> {
        self.pak_collection.as_ref()
    }

    pub fn pak_file_count(&self) -> usize {
        self.pak_collection
            .as_ref()
            .map(|c| c.path_hashes.len())
            .unwrap_or(0)
    }

    pub fn with_filter(mut self, filter: Box<dyn Filter + Send + Sync>) -> Self {
        self.filter = Some(filter);
        self
    }

    pub fn with_magic_filter(mut self, filter: Box<dyn Filter + Send + Sync>) -> Self {
        self.filter = Some(filter);
        self
    }

    fn should_skip_file(&self, data: &[u8], file_hash: Option<u64>) -> bool {
        if let Some(filter) = &self.filter {
            let context = FileContext {
                file_size: data.len() as u64,
                file_hash,
                data: data.to_vec(),
            };
            filter.should_skip_file(&context).unwrap_or_default()
        } else {
            false
        }
    }
}

impl<R> PathSearcher<R>
where
    R: Read + Seek + Send,
{
    pub fn search_memory_dump(&self, dmp_path: &str) -> eyre::Result<SearchResult> {
        fn no_op_progress(_current: u64, _total: u64) {}
        self.search_memory_dump_with_progress(dmp_path, no_op_progress)
    }

    pub fn search_memory_dump_with_progress<P>(
        &self,
        dmp_path: &str,
        progress: P,
    ) -> eyre::Result<SearchResult>
    where
        P: ProgressCallback + Send + Sync,
    {
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

        let mut memory_blocks: Vec<Block> = Vec::with_capacity(memory.len());
        for piece in memory {
            if let Some(prev) = memory_blocks.last_mut()
                && prev.base + prev.len == piece.base_address
            {
                // Only convert to owned when necessary
                if matches!(prev.data, Cow::Borrowed(_)) {
                    let mut owned = prev.data.to_vec();
                    owned.extend(piece.bytes);
                    prev.data = Cow::Owned(owned);
                } else {
                    prev.data.to_mut().extend(piece.bytes);
                }
                prev.len += piece.size;
                continue;
            }
            memory_blocks.push(Block {
                base: piece.base_address,
                len: piece.size,
                data: Cow::Borrowed(piece.bytes),
            })
        }

        progress.on_progress(0, memory_blocks.len() as u64);

        let processed = Mutex::new(0u64);
        all_paths.par_extend(
            memory_blocks
                .par_iter()
                .map(|memory| {
                    let result = if self.should_skip_file(&memory.data, None) {
                        Ok(vec![])
                    } else {
                        self.search_memory(&memory.data, &unk_paths)
                    };
                    let mut count = processed.lock();
                    *count += 1;
                    progress.on_progress(*count, memory_blocks.len() as u64);
                    result
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

    pub fn search_pak_files(&self) -> eyre::Result<SearchResult> {
        fn no_op_progress(_current: u64, _total: u64) {}
        self.search_pak_files_with_progress(no_op_progress)
    }

    pub fn search_pak_files_with_progress<P>(&self, progress: P) -> eyre::Result<SearchResult>
    where
        P: ProgressCallback + Send + Sync,
    {
        let Some(pak_collection) = &self.pak_collection else {
            return Ok(SearchResult::default());
        };

        let mut all_paths: Vec<(String, Vec<I18nPakFileInfo>)> = vec![];
        let unk_paths = Mutex::new(FxHashSet::default());

        let indexes = pak_collection.path_hashes.clone();
        let total_files = indexes.len() as u64;

        progress.on_progress(0, total_files);

        let processed = AtomicU64::new(0);
        all_paths.par_extend(
            indexes
                .keys()
                .par_bridge()
                .map(|hash| {
                    let result = pak_collection.read_file_by_hash(*hash).and_then(|file| {
                        if self.should_skip_file(&file, Some(*hash)) {
                            Ok(vec![])
                        } else {
                            self.search_memory(&file, &unk_paths)
                        }
                    });

                    let count = processed.fetch_add(1, Ordering::Relaxed);
                    progress.on_progress(count, total_files);

                    result
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
                let prior = begin - 2;
                if !accept_char(memory[prior]) {
                    break;
                }
                if memory[prior + 1] != 0 {
                    break;
                }
                begin = prior;
            }
            if begin == slash_pos {
                continue;
            }

            let mut end = slash_pos + 2;
            loop {
                if end >= memory.len() - 1 {
                    break;
                }
                let next = end;
                if !accept_char(memory[next]) {
                    break;
                }
                if memory[next + 1] != 0 {
                    break;
                }
                end = next + 2;
            }
            if end == slash_pos {
                continue;
            }
            pos = (end + 2).min(memory.len());

            let Some(path) = utils::string_from_utf16_bytes(&memory[begin..end]) else {
                continue;
            };

            if !validate_path(&path) {
                continue;
            }

            if let Some(pak) = &self.pak_collection {
                // Check cache first
                if let Some(cached_result) = self.path_cache.get(&path) {
                    // Cache hit
                    if let Some(cached_result) = cached_result.value() {
                        paths.push((path, cached_result.clone()));
                    } else {
                        // If stores None, then ignore
                    }
                    continue;
                }

                // Perform lookup
                let Ok(file_hashes) = suffix::find_path_i18n(pak, &path) else {
                    // No result
                    unk_paths.lock().insert(path.clone());
                    // Also cache empty result
                    self.path_cache.insert(path, None);
                    continue;
                };

                // Cache the result
                self.path_cache
                    .insert(path.clone(), Some(file_hashes.clone()));
                paths.push((path, file_hashes));
            } else {
                paths.push((path, vec![]));
            }
        }

        Ok(paths)
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
    // Quick length check first
    if path.len() < 3 {
        return false;
    }

    let Some((_, tail)) = path.rsplit_once('/') else {
        return false;
    };

    let Some(dot_pos) = tail.find('.') else {
        return false;
    };

    // dot must be in the middle
    dot_pos > 0 && dot_pos < tail.len() - 1
}

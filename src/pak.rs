use std::{fs::File, io::Read, path::Path};

use color_eyre::eyre::{self, Context};
use ree_pak_core::{PakFile, PakReader, utf16_hash::Utf16HashExt};
use rustc_hash::{FxHashMap, FxHashSet};

/// Multiple PAK archive collection.
pub struct PakCollection<R: PakReader> {
    entry_hashes: FxHashSet<u64>,
    last_pak_for_hash: FxHashMap<u64, usize>,
    pak_files: Vec<PakFile<R>>,
}

impl<R> PakCollection<R>
where
    R: PakReader,
{
    pub fn from_readers(readers: Vec<R>) -> eyre::Result<Self> {
        let mut pak_files = Vec::with_capacity(readers.len());
        let mut entry_hashes = FxHashSet::default();
        let mut last_pak_for_hash = FxHashMap::default();

        for (index, reader) in readers.into_iter().enumerate() {
            let pak_file = PakFile::from_reader(reader)?;
            for entry in pak_file.metadata().entries().iter() {
                let hash = entry.hash();
                entry_hashes.insert(hash);
                last_pak_for_hash.insert(hash, index);
            }

            pak_files.push(pak_file);
        }

        Ok(Self {
            pak_files,
            entry_hashes,
            last_pak_for_hash,
        })
    }

    pub fn pak_files(&self) -> &[PakFile<R>] {
        &self.pak_files
    }

    pub fn unique_entry_count(&self) -> usize {
        self.entry_hashes.len()
    }

    /// Return true if this `(pak_index, hash)` is the chosen “winner” for scanning.
    ///
    /// This matches the previous behavior where later PAKs overwrite earlier ones in the hash index.
    pub fn should_scan_hash_in_pak(&self, hash: u64, pak_index: usize) -> bool {
        self.last_pak_for_hash.get(&hash).copied() == Some(pak_index)
    }
}

pub fn load_pak_files_to_memory(paths: &[impl AsRef<Path>]) -> eyre::Result<Vec<Vec<u8>>> {
    let mut pak_data = Vec::with_capacity(paths.len());

    for path in paths.iter() {
        let path = path.as_ref();
        let mut file = File::open(path).context("Failed to open pak file")?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .context("Failed to read pak file")?;
        pak_data.push(data);
    }

    Ok(pak_data)
}

impl<R> PakCollection<R>
where
    R: PakReader,
{
    pub fn contains_path(&self, path: &str) -> bool {
        let hash = path.hash_mixed();
        self.entry_hashes.contains(&hash)
    }
}

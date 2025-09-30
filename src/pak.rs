use std::{
    fs::File,
    io::{self, Read, Seek},
    path::Path,
};

use color_eyre::eyre::{self, Context, ContextCompat};
use parking_lot::Mutex;
use ree_pak_core::{read::archive::PakArchiveReader, utf16_hash::Utf16HashExt};
use rustc_hash::FxHashMap;

#[derive(Clone)]
pub struct PakFileIndex {
    archive_index: usize,
    entry_index: usize,
}

/// Multiple PAK archive collection.
pub struct PakCollection<'a, R> {
    pub path_hashes: FxHashMap<u64, PakFileIndex>,
    pak_readers: Mutex<Vec<PakArchiveReader<'a, R>>>,
}

impl<'a, R> PakCollection<'a, R>
where
    R: Read + Seek,
{
    pub fn from_readers(readers: Vec<R>) -> eyre::Result<Self> {
        let mut pak_readers = Vec::with_capacity(readers.len());
        let mut path_hashes = FxHashMap::default();

        for (index, mut reader) in readers.into_iter().enumerate() {
            let pak_archive = ree_pak_core::read::read_archive(&mut reader)?;
            for (entry_index, entry) in pak_archive.entries().iter().enumerate() {
                path_hashes.insert(
                    entry.hash(),
                    PakFileIndex {
                        archive_index: index,
                        entry_index,
                    },
                );
            }

            let archive_reader = PakArchiveReader::new_owned(reader, pak_archive);
            pak_readers.push(archive_reader);
        }

        Ok(Self {
            pak_readers: Mutex::new(pak_readers),
            path_hashes,
        })
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

impl<R> PakCollection<'_, R> {
    pub fn contains_path(&self, path: &str) -> bool {
        let hash = path.hash_mixed();
        self.path_hashes.contains_key(&hash)
    }
}

impl<R> PakCollection<'_, R>
where
    R: io::Read + io::Seek,
{
    pub fn read_file_by_hash(&self, hash: u64) -> eyre::Result<Vec<u8>> {
        let file_info = self
            .path_hashes
            .get(&hash)
            .context("File not found in any pak")?;

        let mut _pak_readers = self.pak_readers.lock();
        let reader = &mut _pak_readers[file_info.archive_index];
        let mut entry_reader = reader.owned_entry_reader_by_index(file_info.entry_index)?;

        let mut buf = vec![];
        entry_reader.read_to_end(&mut buf)?;

        Ok(buf)
    }
}

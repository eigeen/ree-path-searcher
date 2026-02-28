use color_eyre::eyre::{self, ContextCompat};
use ree_pak_core::PakReader;

use crate::config::PathSearcherConfig;
use crate::pak;
use crate::path_components::PathComponents;

#[derive(Debug, Clone)]
pub struct I18nPakFileInfo {
    pub full_path: String,
}

pub fn find_path_i18n<R: PakReader>(
    pak: &pak::PakCollection<R>,
    config: &PathSearcherConfig,
    parts: &PathComponents<'_>,
) -> eyre::Result<Vec<I18nPakFileInfo>> {
    let raw_path = parts.raw_path();
    let ext = parts.extension().context("Path missing extension")?;
    let versions = config
        .suffix_versions(ext)
        .context(format!("Unknown extension: {ext}"))?;
    for &version in versions.iter().rev() {
        let mut result = vec![];

        let mut candidates = Vec::with_capacity(config.prefixes().len() * (2 + config.platform_suffixes().len()));
        for prefix in config.prefixes() {
            let base = format!("{prefix}{raw_path}.{version}");
            candidates.push(base.clone());
            for suffix in config.platform_suffixes() {
                candidates.push(format!("{base}.{suffix}"));
            }
        }

        for full_path in &candidates {
            // Check base path first (no language suffix)
            if pak.contains_path(full_path) {
                result.push(I18nPakFileInfo {
                    full_path: full_path.clone(),
                });
            }

            // Then check with language suffixes
            for language in config.languages() {
                let with_language = format!("{full_path}.{language}");
                if pak.contains_path(&with_language) {
                    result.push(I18nPakFileInfo {
                        full_path: with_language,
                    });
                }
            }
        }

        if !result.is_empty() {
            // try to find streaming file
            let mut streaming_result = vec![];
            for info in &result {
                let mut pos = 0;
                for prefix in config.prefixes() {
                    if let Some(prefix_pos) = info.full_path.find(prefix.as_str()) {
                        pos = prefix_pos + prefix.len();
                        break;
                    }
                }
                if pos > 0 {
                    let mut streaming_path = info.full_path.clone();
                    streaming_path.insert_str(pos, "streaming/");
                    if pak.contains_path(&streaming_path) {
                        streaming_result.push(I18nPakFileInfo {
                            full_path: streaming_path,
                        });
                    }
                }
            }
            result.extend(streaming_result);

            return Ok(result);
        }
    }

    Ok(vec![])
}

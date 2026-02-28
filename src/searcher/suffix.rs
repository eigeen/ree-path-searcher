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
    let path = parts.raw_path();
    let ext = parts.extension().context("Path missing extension")?;
    let suffix = config
        .suffix_versions(ext)
        .context(format!("Unknown extension: {ext}"))?;
    for suffix in suffix.iter().rev() {
        let mut result = vec![];
        let full_paths = [
            format!("natives/STM/{path}.{suffix}"),
            format!("natives/STM/{path}.{suffix}.X64"),
            format!("natives/STM/{path}.{suffix}.STM"),
            #[cfg(feature = "nsw")]
            format!("natives/NSW/{path}.{suffix}"),
            #[cfg(feature = "nsw")]
            format!("natives/NSW/{path}.{suffix}.NSW"),
            #[cfg(feature = "msg")]
            format!("natives/MSG/{path}.{suffix}"),
            #[cfg(feature = "msg")]
            format!("natives/MSG/{path}.{suffix}.X64"),
            #[cfg(feature = "msg")]
            format!("natives/MSG/{path}.{suffix}.MSG"),
        ];

        for full_path in &full_paths {
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

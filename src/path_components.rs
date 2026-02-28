use std::fmt::Write as _;
use std::ops::Range;

use crate::config::PathSearcherConfig;

#[derive(Debug, Clone)]
pub struct PathComponents<'a> {
    normalized_full: String,
    raw_path: Range<usize>,
    config: &'a PathSearcherConfig,
}

impl<'a> PathComponents<'a> {
    pub fn parse(line: &str, config: &'a PathSearcherConfig) -> Option<Self> {
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            return None;
        }

        let normalized_full = if s.contains('\\') {
            s.replace('\\', "/")
        } else {
            s.to_string()
        };

        let (normalized_full, raw_path) = parse_raw_path_range(normalized_full, config);

        Some(Self {
            normalized_full,
            raw_path,
            config,
        })
    }

    pub fn normalized_full_path(&self) -> &str {
        self.normalized_full.as_str()
    }

    pub fn into_normalized_full_path(self) -> String {
        self.normalized_full
    }

    pub fn has_version(&self) -> bool {
        self.version_range().is_some()
    }

    pub fn prefix(&self) -> Option<&str> {
        self.config.prefixes().iter().find_map(|p| {
            if starts_with_ignore_ascii_case(self.normalized_full.as_str(), p.as_str()) {
                Some(&self.normalized_full[..p.len()])
            } else {
                None
            }
        })
    }

    pub fn set_prefix_str(&mut self, new_prefix: Option<&str>) -> bool {
        let old_len = self
            .config
            .prefixes()
            .iter()
            .find(|p| starts_with_ignore_ascii_case(self.normalized_full.as_str(), p.as_str()))
            .map(|p| p.len())
            .unwrap_or(0);

        let canonical_new = match new_prefix {
            None => "",
            Some(p) => {
                let mut normalized = if p.contains('\\') {
                    p.replace('\\', "/")
                } else {
                    p.to_string()
                };
                while normalized.starts_with('@') || normalized.starts_with('/') {
                    normalized.remove(0);
                }
                if !normalized.is_empty() && !normalized.ends_with('/') {
                    normalized.push('/');
                }
                let Some(canonical) = self
                    .config
                    .prefixes()
                    .iter()
                    .find(|allowed| allowed.as_str().eq_ignore_ascii_case(normalized.as_str()))
                    .map(|s| s.as_str())
                else {
                    return false;
                };
                canonical
            }
        };

        self.normalized_full
            .replace_range(0..old_len, canonical_new);
        self.recompute_raw_path_range();
        true
    }

    pub fn raw_path(&self) -> &str {
        &self.normalized_full[self.raw_path.clone()]
    }

    pub fn raw_path_range(&self) -> Range<usize> {
        self.raw_path.clone()
    }

    pub fn set_raw_path_str(&mut self, new_raw_path: &str) -> bool {
        let mut normalized = if new_raw_path.contains('\\') {
            new_raw_path.replace('\\', "/")
        } else {
            new_raw_path.to_string()
        };
        while normalized.starts_with('@') || normalized.starts_with('/') {
            normalized.remove(0);
        }
        if normalized.is_empty() {
            return false;
        }

        let old_range = self.raw_path.clone();
        self.normalized_full
            .replace_range(old_range, normalized.as_str());
        self.recompute_raw_path_range();
        true
    }

    pub fn version_str(&self) -> Option<&str> {
        self.version_range()
            .map(|r| &self.normalized_full[r.clone()])
    }

    pub fn version_range(&self) -> Option<Range<usize>> {
        let dot = self.raw_path.end;
        if dot >= self.normalized_full.len() {
            return None;
        }
        if self.normalized_full.as_bytes().get(dot) != Some(&b'.') {
            return None;
        }

        let start = dot + 1;
        let end = self.normalized_full[start..]
            .find('.')
            .map(|rel| start + rel)
            .unwrap_or(self.normalized_full.len());

        let seg = &self.normalized_full[start..end];
        if is_digits(seg) {
            Some(start..end)
        } else {
            None
        }
    }

    pub fn set_version_u32(&mut self, version: u32) -> bool {
        let mut s = String::new();
        let _ = write!(&mut s, "{version}");
        self.set_version_str(s.as_str())
    }

    pub fn set_version_str(&mut self, new_version: &str) -> bool {
        if new_version.is_empty() || !new_version.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }

        if let Some(old_range) = self.version_range() {
            self.normalized_full.replace_range(old_range, new_version);
            self.recompute_raw_path_range();
            return true;
        }

        self.normalized_full.push('.');
        self.normalized_full.push_str(new_version);
        self.recompute_raw_path_range();
        true
    }

    pub fn clear_version(&mut self) -> bool {
        if self.version_range().is_none() {
            return false;
        }
        let dot = self.raw_path.end;
        self.normalized_full.truncate(dot);
        self.recompute_raw_path_range();
        true
    }

    pub fn platform(&self) -> Option<&str> {
        self.tag_str(TagKind::Platform)
    }

    pub fn set_platform_str(&mut self, new_platform: Option<&str>) -> bool {
        let canonical = match new_platform {
            None => None,
            Some(s) if s.eq_ignore_ascii_case("STM") => Some("STM"),
            Some(s) if s.eq_ignore_ascii_case("NSW") => Some("NSW"),
            Some(s) if s.eq_ignore_ascii_case("MSG") => Some("MSG"),
            Some(_) => return false,
        };
        self.set_tag(TagKind::Platform, canonical)
    }

    pub fn language(&self) -> Option<&str> {
        self.tag_str(TagKind::Language)
    }

    pub fn set_language_str(&mut self, new_language: Option<&str>) -> bool {
        let canonical = match new_language {
            None => None,
            Some(s) => self
                .config
                .languages()
                .iter()
                .find(|lang| lang.as_str().eq_ignore_ascii_case(s))
                .map(|lang| lang.as_str())
                .or_else(|| {
                    if self.config.languages().iter().any(|lang| lang == s) {
                        Some(s)
                    } else {
                        None
                    }
                }),
        };
        if new_language.is_some() && canonical.is_none() {
            return false;
        }
        self.set_tag(TagKind::Language, canonical)
    }

    pub fn arch(&self) -> Option<&str> {
        self.tag_str(TagKind::Arch)
    }

    pub fn set_arch_str(&mut self, new_arch: Option<&str>) -> bool {
        let canonical = match new_arch {
            None => None,
            Some(s) if s.eq_ignore_ascii_case("X64") => Some("X64"),
            Some(_) => return false,
        };
        self.set_tag(TagKind::Arch, canonical)
    }

    pub fn extension(&self) -> Option<&str> {
        let raw = self.raw_path();
        let dot = raw.rfind('.')?;
        Some(&raw[dot + 1..])
    }

    fn recompute_raw_path_range(&mut self) {
        let normalized_full = std::mem::take(&mut self.normalized_full);
        let (normalized_full, raw_path) = parse_raw_path_range(normalized_full, self.config);
        self.normalized_full = normalized_full;
        self.raw_path = raw_path;
    }

    fn tag_str(&self, kind: TagKind) -> Option<&str> {
        let version_range = self.version_range()?;
        for r in self.tag_ranges_after_version(version_range) {
            let s = &self.normalized_full[r.clone()];
            if classify_tag(self.config, s) == kind {
                return Some(s);
            }
        }
        None
    }

    fn set_tag(&mut self, kind: TagKind, new_value: Option<&str>) -> bool {
        let Some(version_range) = self.version_range() else {
            return false;
        };

        let mut tags: Vec<String> = self
            .tag_ranges_after_version(version_range.clone())
            .into_iter()
            .map(|r| self.normalized_full[r].to_string())
            .collect();

        let mut out = Vec::with_capacity(tags.len() + 1);
        let mut replaced = false;
        for t in tags.drain(..) {
            if classify_tag(self.config, t.as_str()) == kind {
                if let Some(v) = new_value {
                    if !replaced {
                        out.push(v.to_string());
                        replaced = true;
                    }
                } else {
                    // clear
                }
            } else {
                out.push(t);
            }
        }

        if !replaced {
            if let Some(v) = new_value {
                out.push(v.to_string());
            }
        }

        let base = &self.normalized_full[..self.raw_path.end];
        let version = &self.normalized_full[version_range];
        let mut rebuilt = String::with_capacity(self.normalized_full.len() + 8);
        rebuilt.push_str(base);
        rebuilt.push('.');
        rebuilt.push_str(version);
        for t in out {
            rebuilt.push('.');
            rebuilt.push_str(t.as_str());
        }

        self.normalized_full = rebuilt;
        self.recompute_raw_path_range();
        true
    }

    fn tag_ranges_after_version(&self, version_range: Range<usize>) -> Vec<Range<usize>> {
        let mut out = vec![];
        let bytes = self.normalized_full.as_bytes();
        let mut i = version_range.end;
        while i < bytes.len() {
            if bytes[i] != b'.' {
                break;
            }
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'.' {
                end += 1;
            }
            if start < end {
                out.push(start..end);
            }
            i = end;
        }
        out
    }
}

fn last_segment_range(s: &str, end: usize) -> Option<(Range<usize>, usize)> {
    let dot = s.get(..end)?.rfind('.')?;
    Some(((dot + 1)..end, dot))
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn is_tag(s: &str, config: &PathSearcherConfig) -> bool {
    is_language_tag(config, s) || is_platform_tag(s) || is_arch_tag(s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Platform,
    Language,
    Arch,
    Unknown,
}

fn classify_tag(config: &PathSearcherConfig, s: &str) -> TagKind {
    if is_platform_tag(s) {
        return TagKind::Platform;
    }
    if is_arch_tag(s) {
        return TagKind::Arch;
    }
    if is_language_tag(config, s) {
        return TagKind::Language;
    }
    TagKind::Unknown
}

fn parse_raw_path_range(
    mut normalized_full: String,
    config: &PathSearcherConfig,
) -> (String, Range<usize>) {
    while normalized_full.starts_with('@') || normalized_full.starts_with('/') {
        normalized_full.remove(0);
    }

    let mut raw_start = 0usize;
    for p in config.prefixes() {
        if starts_with_ignore_ascii_case(normalized_full.as_str(), p.as_str()) {
            raw_start = p.len();
            break;
        }
    }
    if raw_start == 0 {
        for p in config.prefixes() {
            if let Some(pos) = find_ignore_ascii_case(normalized_full.as_str(), p.as_str()) {
                normalized_full.drain(..pos);
                raw_start = p.len();
                break;
            }
        }
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(&normalized_full[raw_start..], "streaming/")
    {
        raw_start = normalized_full.len() - rest.len();
    }

    let mut raw_end = normalized_full.len();
    if let Some((seg_a, dot_a)) = last_segment_range(&normalized_full, normalized_full.len()) {
        let seg_a_str = &normalized_full[seg_a.clone()];

        if is_digits(seg_a_str) {
            // ... .<ext>.<version>
            raw_end = dot_a;
        } else if is_tag(seg_a_str, config) {
            if let Some((seg_b, dot_b)) = last_segment_range(&normalized_full, dot_a) {
                let seg_b_str = &normalized_full[seg_b.clone()];
                if is_digits(seg_b_str) {
                    // ... .<ext>.<version>.<tag>
                    raw_end = dot_b;
                } else if is_tag(seg_b_str, config) {
                    if let Some((seg_c, dot_c)) = last_segment_range(&normalized_full, dot_b) {
                        let seg_c_str = &normalized_full[seg_c.clone()];
                        if is_digits(seg_c_str) {
                            // ... .<ext>.<version>.<tag>.<tag>
                            raw_end = dot_c;
                        }
                    }
                }
            }
        }
    }

    if raw_end < raw_start {
        raw_end = raw_start;
    }

    (normalized_full, raw_start..raw_end)
}

pub fn is_platform_tag(s: &str) -> bool {
    s.eq_ignore_ascii_case("STM") || s.eq_ignore_ascii_case("NSW") || s.eq_ignore_ascii_case("MSG")
}

pub fn is_arch_tag(s: &str) -> bool {
    s.eq_ignore_ascii_case("X64")
}

pub fn is_language_tag(config: &PathSearcherConfig, s: &str) -> bool {
    config
        .languages()
        .iter()
        .any(|lang| lang.as_str().eq_ignore_ascii_case(s))
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

pub fn strip_prefix_ignore_ascii_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay = haystack.as_bytes();
    let ned = needle.as_bytes();
    if ned.len() > hay.len() {
        return None;
    }
    hay.windows(ned.len()).position(|window| {
        window
            .iter()
            .zip(ned.iter())
            .all(|(&a, &b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_version_keeps_raw_path() {
        let cfg = PathSearcherConfig::default();
        let mut p = PathComponents::parse(
            "natives/stm/systems/rendering/bluenoise256x256/hdr_rgba_0028.tex.251111100",
            &cfg,
        )
        .unwrap();

        assert_eq!(
            p.raw_path(),
            "systems/rendering/bluenoise256x256/hdr_rgba_0028.tex"
        );
        assert_eq!(p.version_str(), Some("251111100"));

        assert!(p.set_version_str("241106027"));
        assert_eq!(
            p.normalized_full_path(),
            "natives/stm/systems/rendering/bluenoise256x256/hdr_rgba_0028.tex.241106027"
        );
        assert_eq!(
            p.raw_path(),
            "systems/rendering/bluenoise256x256/hdr_rgba_0028.tex"
        );
        assert_eq!(p.version_str(), Some("241106027"));
    }

    #[test]
    fn test_set_and_clear_tags() {
        let cfg = PathSearcherConfig::default();
        let mut p = PathComponents::parse(
            "natives/STM/systems/rendering/bluenoise256x256/hdr_rgba_0028.tex.241106027",
            &cfg,
        )
        .unwrap();

        assert!(p.set_arch_str(Some("x64")));
        assert_eq!(
            p.normalized_full_path(),
            "natives/STM/systems/rendering/bluenoise256x256/hdr_rgba_0028.tex.241106027.X64"
        );
        assert_eq!(p.arch(), Some("X64"));

        assert!(p.set_language_str(Some("ja")));
        assert_eq!(p.language(), Some("Ja"));

        assert!(p.set_arch_str(None));
        assert_eq!(p.arch(), None);

        assert!(p.clear_version());
        assert_eq!(
            p.normalized_full_path(),
            "natives/STM/systems/rendering/bluenoise256x256/hdr_rgba_0028.tex"
        );
        assert_eq!(p.version_str(), None);
        assert_eq!(p.language(), None);
    }
}

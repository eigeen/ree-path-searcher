use std::fs;
use std::path::Path;
use std::sync::Arc;

use color_eyre::eyre::{self, Context};
use rustc_hash::FxHashMap;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct PathSearcherConfig {
    languages: Arc<[String]>,
    prefixes: Arc<[String]>,
    suffix_map: Arc<FxHashMap<String, Vec<u32>>>,
}

#[derive(Debug, Clone, Deserialize)]
struct PathSearcherConfigFile {
    pub languages: Option<Vec<String>>,
    pub prefixes: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub use_builtin_suffix_map: bool,
    #[serde(default)]
    pub suffix_map: FxHashMap<String, Vec<u32>>,
    // Back-compat for older config keys (kept merged into `suffix_map` at load time).
    #[serde(default)]
    pub suffix_map_overrides: FxHashMap<String, Vec<u32>>,
    #[serde(default)]
    pub suffix_map_full: FxHashMap<String, Vec<u32>>,
}

impl Default for PathSearcherConfig {
    fn default() -> Self {
        Self {
            languages: default_languages().into(),
            prefixes: default_prefixes().into(),
            suffix_map: Arc::new(default_suffix_map_full_owned()),
        }
    }
}

impl PathSearcherConfig {
    pub fn from_toml_str(toml_str: &str) -> eyre::Result<Self> {
        let file_cfg: PathSearcherConfigFile = toml::from_str(toml_str)?;
        Ok(Self::from_file_config(file_cfg))
    }

    pub fn from_toml_file(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let path = path.as_ref();
        let s = fs::read_to_string(path)
            .wrap_err_with(|| format!("Failed to read config: {path:?}"))?;
        Self::from_toml_str(&s).wrap_err_with(|| format!("Failed to parse TOML config: {path:?}"))
    }

    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    pub fn prefixes(&self) -> &[String] {
        &self.prefixes
    }

    pub fn suffix_versions(&self, extension: &str) -> Option<&[u32]> {
        self.suffix_map.get(extension).map(Vec::as_slice)
    }

    fn from_file_config(file_cfg: PathSearcherConfigFile) -> Self {
        let languages: Arc<[String]> = file_cfg.languages.unwrap_or_else(default_languages).into();
        let prefixes: Arc<[String]> = file_cfg.prefixes.unwrap_or_else(default_prefixes).into();

        let mut suffix_map = if file_cfg.use_builtin_suffix_map {
            default_suffix_map_full_owned()
        } else {
            FxHashMap::default()
        };

        // New key (preferred).
        suffix_map.extend(file_cfg.suffix_map);
        // Old keys (merge/override).
        suffix_map.extend(file_cfg.suffix_map_full);
        suffix_map.extend(file_cfg.suffix_map_overrides);

        Self {
            languages,
            prefixes,
            suffix_map: Arc::new(suffix_map),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_languages() -> Vec<String> {
    vec![
        "Ja", "En", "Fr", "It", "De", "Es", "Ru", "Pl", "Nl", "Pt", "PtBR", "Ko", "ZhTW", "ZhCN",
        "Fi", "Sv", "Da", "No", "Cs", "Hu", "Sk", "Ar", "Tr", "Bu", "Gr", "Ro", "Th", "Uk", "Vi",
        "Id", "Fc", "Hi", "Es419",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_prefixes() -> Vec<String> {
    let mut prefixes = Vec::with_capacity(3);
    prefixes.push("natives/STM/".to_string());
    #[cfg(feature = "nsw")]
    prefixes.push("natives/NSW/".to_string());
    #[cfg(feature = "msg")]
    prefixes.push("natives/MSG/".to_string());
    prefixes
}

fn default_suffix_map_full_owned() -> FxHashMap<String, Vec<u32>> {
    default_suffix_map_full()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_vec()))
        .collect()
}

// The suffix list for a particular file format is ordered that the first version comes first.
fn default_suffix_map_full() -> FxHashMap<&'static str, &'static [u32]> {
    // Base version: mhwilds
    FxHashMap::from_iter([
        ("abcmesh", &[12][..]),
        ("aebs", &[3]),
        ("aecr", &[3]),
        ("aedt", &[3]),
        ("aeeq", &[3]),
        ("aefb", &[3]),
        ("aeir", &[3]),
        ("aelp", &[3]),
        ("aemc", &[3]),
        ("aemd", &[3]),
        ("aeme", &[3]),
        ("aeml", &[3]),
        ("aepp", &[3]),
        ("aerb", &[3]),
        ("aesr", &[3]),
        ("aetr", &[3]),
        ("aimap", &[45]),
        ("aimapattr", &[46]),
        ("ainvm", &[30]),
        ("ainvmmgr", &[8]),
        ("aivspc", &[10]),
        ("aivspcmgr", &[8]),
        ("aiwayp", &[9]),
        ("aiwaypmgr", &[8]),
        ("amix", &[3]),
        ("apsrc", &[21]),
        ("arexprgraph", &[6]),
        ("asrc", &[34]),
        ("auto", &[4]),
        ("bhvt", &[41]),
        ("capface", &[1]),
        ("caphand", &[2]),
        ("ccbk", &[3]),
        ("cdef", &[7]),
        ("cfil", &[7]),
        ("chain", &[55]),
        ("chain2", &[12, 13]),
        ("chainwnd", &[0]),
        ("chf", &[2]),
        ("clip", &[82, 85]),
        ("cloth2", &[240820144, 241111607]),
        ("clrp", &[1]),
        ("clsm", &[17]),
        ("clsp", &[3]),
        ("cmat", &[3]),
        ("coco", &[10]),
        ("csdf", &[240718144, 240906212, 250206177]),
        ("cset", &[6]),
        ("dblc", &[1]),
        ("def", &[6]),
        ("dlg", &[30011]),
        ("dlgcf", &[1]),
        ("dlglist", &[30007]),
        ("dlgtml", &[82002, 85002]),
        ("dlgtmllist", &[82002000, 85002000]),
        ("ecob", &[1]),
        ("eem", &[0]),
        ("efcsv", &[1]),
        ("efx", &[5375364, 5571972]),
        ("emesh", &[1]),
        ("exprgraph", &[5]),
        ("fbik", &[6]),
        ("fbxskel", &[7]),
        ("fgrl", &[1]),
        ("filter", &[1]),
        ("finf", &[2]),
        ("fol", &[240718001]),
        ("fpolygon", &[30001]),
        ("fslt", &[4]),
        ("fsmv2", &[41]),
        ("fxct", &[4]),
        ("gcf", &[28]),
        ("gclo", &[240820217, 241111681, 241111688, 241111689]),
        ("gcp", &[2]),
        ("gml", &[240701013, 241106040]),
        ("gp", &[0]),
        ("gpbf", &[3]),
        ("gpuc", &[240820252, 241111720, 241111734, 241111744]),
        ("gpumotlist", &[903, 934]),
        ("gpus", &[10]),
        ("grnd", &[240701027, 241106053]),
        ("gsty", &[4]),
        ("gtex", &[240701004, 241106030]),
        ("gtl", &[240701019, 241106045]),
        ("gui", &[820041, 850041]),
        ("guisd", &[1]),
        ("hapvib", &[1807190270]),
        ("hf", &[4]),
        ("htex", &[1]),
        ("ies", &[2]),
        ("ift", &[7]),
        ("ik3dpath", &[1]),
        ("ikbodyrig", &[3]),
        ("ikdamage", &[4]),
        ("ikfs", &[3]),
        ("ikhd", &[5]),
        ("ikleg2", &[24]),
        ("iklizard", &[6]),
        ("iklookat", &[2]),
        ("iklookat2", &[26]),
        ("ikls", &[28]),
        ("ikmulti", &[4]),
        ("ikspinecg", &[1]),
        ("iktrain", &[5]),
        ("iktrain2", &[1]),
        ("ikwagon", &[1]),
        ("jcns", &[28, 29]),
        ("jmap", &[26]),
        ("jntexprgraph", &[6]),
        ("jointlodgroup", &[2]),
        ("jointsetting", &[1]),
        ("lfa", &[4]),
        ("lform", &[7]),
        ("lmap", &[481028330, 481433356]),
        ("lod", &[3]),
        ("lprb", &[8]),
        ("maba", &[3]),
        ("mcambank", &[3]),
        ("mcamlist", &[22]),
        ("mcol", &[24022]),
        ("mdf2", &[45]),
        ("mesh", &[240820143, 241111606]),
        ("mmtr", &[240718143, 240906211, 250206176]),
        ("mmtrs", &[240718143, 240906211, 250206176]),
        ("mot", &[901, 932]),
        ("motbank", &[4]),
        ("motblend", &[901, 932]),
        ("motcam", &[12]),
        ("motface", &[27, 28]),
        ("motfsm2", &[44]),
        ("motlist", &[959, 992]),
        ("mottree", &[21]),
        ("mov", &[1]),
        ("mpci", &[240802001, 241003001]),
        ("msg", &[23]),
        ("nar", &[1]),
        ("ncf", &[11]),
        ("nmr", &[18]),
        ("ocioc", &[481419375, 482012469, 491312434]),
        ("oft", &[1]),
        ("ord", &[1]),
        ("particle", &[3]),
        ("path", &[0]),
        ("pci", &[5]),
        ("pfb", &[18]),
        ("pfnn", &[0]),
        ("pog", &[10, 12]),
        ("poglst", &[0]),
        ("prb", &[9]),
        ("prvs", &[1]),
        ("psop", &[3]),
        ("rbs", &[2038]),
        ("rbsl", &[1]),
        ("rcf", &[3]),
        ("rcfg", &[10]),
        ("rcol", &[27, 28]),
        ("rdc", &[2038024003]),
        ("rdd", &[2038024]),
        ("refskel", &[7]),
        ("retarget", &[7]),
        ("retargetfleg", &[1]),
        ("retargetrig", &[9]),
        ("rfl", &[1]),
        ("rmat", &[1]),
        ("rmesh", &[26013]),
        ("road", &[4]),
        ("rtbs", &[5]),
        ("rtex", &[6]),
        ("rtmr", &[7]),
        ("sbd", &[6]),
        ("sbnk", &[1]),
        ("scb", &[1]),
        ("scl", &[1]),
        ("scn", &[21]),
        ("scns", &[1]),
        ("sdf", &[240718143, 240906211, 250206176]),
        ("sdftex", &[480824330, 481229356]),
        ("sfur", &[5]),
        ("skeleton", &[7]),
        ("slqg", &[1]),
        ("smt", &[1]),
        ("spck", &[1]),
        ("spmt", &[4]),
        ("sss", &[5]),
        ("sst", &[10]),
        ("star", &[3]),
        ("stl", &[3]),
        ("stmesh", &[240906225, 241111606]),
        ("strands", &[25]),
        ("sts", &[1]),
        ("svgn", &[4]),
        ("svgsq", &[1]),
        ("svx", &[1]),
        ("swexprgraph", &[6]),
        ("swgm", &[3]),
        ("swid", &[1]),
        ("swms", &[1]),
        ("tean", &[30001]),
        ("terr", &[24008]),
        ("tex", &[240701001, 241106027]),
        ("tml", &[82004, 85004]),
        ("tmlbld", &[82013, 85013]),
        ("tmlfsm2", &[41082004, 41085004]),
        ("trtd", &[3004]),
        ("ucurve", &[83, 86]),
        ("ucurvelist", &[82, 85]),
        ("user", &[3]),
        ("uvar", &[3]),
        ("uvs", &[8]),
        ("vehicle", &[2038017]),
        ("vehicle2", &[2038003]),
        ("vmap", &[240724984]),
        ("vsdf", &[240718147, 240906215, 250206180]),
        ("vsdflist", &[1]),
        ("vsrc", &[21]),
        ("vtxa", &[220513984]),
        ("wrap", &[231020828]),
        ("wsg", &[1]),
        ("ziva", &[240220828]),
        ("zivacomb", &[240321828]),
    ])
}

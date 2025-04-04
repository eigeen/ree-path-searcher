use core::str;
use std::{borrow::Cow, sync::LazyLock};

use color_eyre::eyre::{self, ContextCompat};
use rustc_hash::FxHashMap;

use crate::pak;

pub const LANGUAGE_LIST: &[&str] = &[
    "", "Ja", "En", "Fr", "It", "De", "Es", "Ru", "Pl", "Nl", "Pt", "PtBR", "Ko", "ZhTW", "ZhCN",
    "Fi", "Sv", "Da", "No", "Cs", "Hu", "Sk", "Ar", "Tr", "Bu", "Gr", "Ro", "Th", "Uk", "Vi", "Id",
    "Fc", "Hi", "Es419",
];

// The suffix list for a particular file format is ordered that the first version comes first
pub static SUFFIX_MAP_FULL: LazyLock<FxHashMap<&'static str, &'static [u32]>> =
    LazyLock::new(|| {
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
            ("chain2", &[13]),
            ("chainwnd", &[0]),
            ("chf", &[2]),
            ("clip", &[85]),
            ("cloth2", &[241111607]),
            ("clrp", &[1]),
            ("clsm", &[17]),
            ("clsp", &[3]),
            ("cmat", &[3]),
            ("coco", &[10]),
            ("csdf", &[240906212]),
            ("cset", &[6]),
            ("dblc", &[1]),
            ("def", &[6]),
            ("dlg", &[30011]),
            ("dlgcf", &[1]),
            ("dlglist", &[30007]),
            ("dlgtml", &[85002]),
            ("dlgtmllist", &[85002000]),
            ("ecob", &[1]),
            ("eem", &[0]),
            ("efcsv", &[1]),
            ("efx", &[5571972]),
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
            ("gclo", &[241111681]),
            ("gcp", &[2]),
            ("gml", &[241106040]),
            ("gp", &[0]),
            ("gpbf", &[3]),
            ("gpuc", &[241111720]),
            ("gpumotlist", &[934]),
            ("gpus", &[10]),
            ("grnd", &[241106053]),
            ("gsty", &[4]),
            ("gtex", &[241106030]),
            ("gtl", &[241106045]),
            ("gui", &[850041]),
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
            ("jcns", &[29]),
            ("jmap", &[26]),
            ("jntexprgraph", &[6]),
            ("jointlodgroup", &[2]),
            ("jointsetting", &[1]),
            ("lfa", &[4]),
            ("lform", &[7]),
            ("lmap", &[481433356]),
            ("lod", &[3]),
            ("lprb", &[8]),
            ("maba", &[3]),
            ("mcambank", &[3]),
            ("mcamlist", &[22]),
            ("mcol", &[24022]),
            ("mdf2", &[45]),
            ("mesh", &[241111606]),
            ("mmtr", &[240906211]),
            ("mmtrs", &[240906211]),
            ("mot", &[932]),
            ("motbank", &[4]),
            ("motblend", &[932]),
            ("motcam", &[12]),
            ("motface", &[28]),
            ("motfsm2", &[44]),
            ("motlist", &[992]),
            ("mottree", &[21]),
            ("mov", &[1]),
            ("mpci", &[241003001]),
            ("msg", &[23]),
            ("nar", &[1]),
            ("ncf", &[11]),
            ("nmr", &[18]),
            ("ocioc", &[482012469]),
            ("oft", &[1]),
            ("ord", &[1]),
            ("particle", &[3]),
            ("path", &[0]),
            ("pci", &[5]),
            ("pfb", &[18]),
            ("pfnn", &[0]),
            ("pog", &[12]),
            ("poglst", &[0]),
            ("prb", &[9]),
            ("prvs", &[1]),
            ("psop", &[3]),
            ("rbs", &[2038]),
            ("rbsl", &[1]),
            ("rcf", &[3]),
            ("rcfg", &[10]),
            ("rcol", &[28]),
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
            ("sdf", &[240906211]),
            ("sdftex", &[481229356]),
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
            ("stmesh", &[241111606]),
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
            ("tex", &[241106027]),
            ("tml", &[85004]),
            ("tmlbld", &[85013]),
            ("tmlfsm2", &[41085004]),
            ("trtd", &[3004]),
            ("ucurve", &[86]),
            ("ucurvelist", &[85]),
            ("user", &[3]),
            ("uvar", &[3]),
            ("uvs", &[8]),
            ("vehicle", &[2038017]),
            ("vehicle2", &[2038003]),
            ("vmap", &[240724984]),
            ("vsdf", &[240906215]),
            ("vsdflist", &[1]),
            ("vsrc", &[21]),
            ("vtxa", &[220513984]),
            ("wrap", &[231020828]),
            ("wsg", &[1]),
            ("ziva", &[240220828]),
            ("zivacomb", &[240321828]),
        ])
    });

#[derive(Debug, Clone)]
pub struct I18nPakFileInfo {
    pub full_path: String,
}

pub fn find_path_i18n<R>(
    pak: &pak::PakCollection<R>,
    mut path: &str,
) -> eyre::Result<Vec<I18nPakFileInfo>> {
    if path.starts_with('@') || path.starts_with('/') {
        path = &path[1..];
    }
    const PREFIXES: &[&str] = &[
        "natives/STM/",
        #[cfg(feature = "nsw")]
        "natives/NSW/",
        #[cfg(feature = "msg")]
        "natives/MSG/",
    ];
    // strip prefix
    for prefix in PREFIXES {
        if let Some(prefix_pos) = path.find(prefix) {
            path = &path[(prefix_pos + prefix.len())..];
            break;
        }
    }
    // strip suffix
    let mut dot = path.rfind('.').context("Path missing extension")?;
    let ext = &path[dot + 1..];
    if ext.chars().all(|c| c.is_ascii_digit()) {
        // pure number, is suffix
        path = &path[..dot];
        dot = path.rfind('.').context("Path missing extension")?;
    }

    let suffix = SUFFIX_MAP_FULL
        .get(&path[dot + 1..])
        .context(format!("Unknown extension: {}", &path[dot + 1..]))?;
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

        for &language in LANGUAGE_LIST {
            for full_path in &full_paths {
                let with_language: Cow<'_, str> = if language.is_empty() {
                    Cow::Borrowed(full_path)
                } else {
                    Cow::Owned(format!("{full_path}.{language}"))
                };
                if pak.contains_path(&with_language) {
                    result.push(I18nPakFileInfo {
                        full_path: with_language.to_string(),
                    });
                    break;
                }
            }
        }

        if !result.is_empty() {
            // try to find streaming file
            let mut streaming_result = vec![];
            for info in &result {
                let mut pos = 0;
                for prefix in PREFIXES {
                    if let Some(prefix_pos) = info.full_path.find(prefix) {
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

use std::collections::BTreeMap;
use std::path::Path;

/// Map manifest path → output basename. Collisions on basename are resolved
/// by appending `-2`, `-3`, etc., assigned in sorted manifest-path order.
pub fn resolve_basenames<I, S>(manifest_paths: I) -> BTreeMap<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut sorted: Vec<String> = manifest_paths.into_iter().map(|s| s.as_ref().to_string()).collect();
    sorted.sort();

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut out: BTreeMap<String, String> = BTreeMap::new();

    for path in sorted {
        let basename = Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image")
            .to_string();
        let count = counts.entry(basename.clone()).or_insert(0);
        *count += 1;
        let output = if *count == 1 {
            basename
        } else {
            // foo.jpg → foo-2.jpg
            let (stem, ext) = match basename.rsplit_once('.') {
                Some((s, e)) => (s.to_string(), format!(".{e}")),
                None => (basename.clone(), String::new()),
            };
            format!("{stem}-{count}{ext}")
        };
        out.insert(path, output);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_collision_keeps_basename() {
        let m = resolve_basenames(["OEBPS/images/cat.jpg", "OEBPS/images/dog.jpg"]);
        assert_eq!(m.get("OEBPS/images/cat.jpg").unwrap(), "cat.jpg");
        assert_eq!(m.get("OEBPS/images/dog.jpg").unwrap(), "dog.jpg");
    }

    #[test]
    fn collision_suffixed() {
        let m = resolve_basenames(["OEBPS/images/foo.jpg", "OEBPS/figs/foo.jpg"]);
        assert_eq!(m.get("OEBPS/figs/foo.jpg").unwrap(), "foo.jpg");
        assert_eq!(m.get("OEBPS/images/foo.jpg").unwrap(), "foo-2.jpg");
    }

    #[test]
    fn three_way_collision_in_sorted_order() {
        let m = resolve_basenames([
            "z/foo.png", "a/foo.png", "m/foo.png",
        ]);
        assert_eq!(m.get("a/foo.png").unwrap(), "foo.png");
        assert_eq!(m.get("m/foo.png").unwrap(), "foo-2.png");
        assert_eq!(m.get("z/foo.png").unwrap(), "foo-3.png");
    }

    #[test]
    fn no_extension() {
        let m = resolve_basenames(["a/x", "b/x"]);
        assert_eq!(m.get("a/x").unwrap(), "x");
        assert_eq!(m.get("b/x").unwrap(), "x-2");
    }
}

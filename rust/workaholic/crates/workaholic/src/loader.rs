use std::path::Path;

use crate::{document::RawDocument, error::Result, traits::DocumentsLoader};

/// Loads YAML, YAML multi-doc, and JSON documents from a local directory tree
/// or a single file.
pub struct LocalDocumentLoader;

impl DocumentsLoader for LocalDocumentLoader {
    fn load(&self, path: &Path) -> Result<Vec<RawDocument>> {
        let mut docs = Vec::new();
        if path.is_file() {
            docs.extend(load_file(path)?);
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let p = entry.path();
                if matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("yaml" | "yml" | "json")
                ) {
                    match load_file(p) {
                        Ok(d) => docs.extend(d),
                        Err(e) => log::warn!("[loader] skipping {:?}: {e}", p),
                    }
                }
            }
        }
        Ok(docs)
    }
}

fn load_file(path: &Path) -> Result<Vec<RawDocument>> {
    let content = std::fs::read_to_string(path)?;
    let source = path.to_string_lossy().to_string();

    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        let mut doc: RawDocument = serde_json::from_str(&content)?;
        doc.source_path = Some(source);
        return Ok(vec![doc]);
    }

    // YAML: support multi-document files separated by `---`
    let mut result = Vec::new();
    for (i, part) in content.split("\n---").enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match serde_yaml::from_str::<RawDocument>(part) {
            Ok(mut doc) => {
                doc.source_path = Some(if i == 0 {
                    source.clone()
                } else {
                    format!("{source}[{i}]")
                });
                result.push(doc);
            }
            Err(e) => {
                log::warn!("[loader] YAML parse error in {:?}: {e}", path);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_yaml_file() {
        let mut f = NamedTempFile::with_suffix(".yaml").unwrap();
        writeln!(
            f,
            "kind: orkester/namespace:1.0\nname: test\nversion: 1.0.0\nspec: {{}}"
        )
        .unwrap();
        let loader = LocalDocumentLoader;
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].name, "test");
    }

    #[test]
    fn load_multidoc_yaml() {
        let mut f = NamedTempFile::with_suffix(".yaml").unwrap();
        writeln!(
            f,
            "kind: orkester/namespace:1.0\nname: ns1\nversion: 1.0.0\nspec: {{}}\n\n---\nkind: orkester/namespace:1.0\nname: ns2\nversion: 1.0.0\nspec: {{}}"
        )
        .unwrap();
        let loader = LocalDocumentLoader;
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs.len(), 2);
    }
}

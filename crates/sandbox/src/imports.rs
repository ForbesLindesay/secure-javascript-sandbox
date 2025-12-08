use std::{path::Path, sync::Arc};

pub enum ResolvedModule {
    Url(String),
    Id(String),
}

pub trait CustomImportMap: Send + Sync + 'static {
    fn resolve_import_path(&self, path: String, parent: String) -> anyhow::Result<ResolvedModule>;
    fn load_import(&self, id: String) -> anyhow::Result<String>;
}

pub(crate) struct ImportMapBlockAll;
impl CustomImportMap for ImportMapBlockAll {
    fn resolve_import_path(&self, _path: String, _parent: String) -> anyhow::Result<ResolvedModule> {
        Err(anyhow::anyhow!("Importing modules is blocked"))
    }
    fn load_import(&self, _id: String) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Importing modules is blocked"))
    }
}

#[derive(Clone)]
pub enum ImportMap {
    AllowHttp,
    AllowFolder(Arc<Path>),
    BlockAll,
}
impl Default for ImportMap {
    fn default() -> Self {
        ImportMap::AllowHttp
    }
}
impl CustomImportMap for ImportMap {
    fn resolve_import_path(&self, path: String, parent: String) -> anyhow::Result<ResolvedModule> {
        match self {
            ImportMap::AllowHttp => Ok(ResolvedModule::Url(path)),
            ImportMap::AllowFolder(folder) => {
                if parent == "<main>" {
                    let full_path = folder.join(&path);
                    Ok(ResolvedModule::Id(full_path.to_string_lossy().to_string()))
                } else {
                    let parent_path = Path::new(&parent);
                    let parent_dir = parent_path.parent().ok_or_else(|| anyhow::anyhow!("Parent path has no parent directory"))?;
                    let full_path = parent_dir.join(&path);
                    Ok(ResolvedModule::Id(full_path.to_string_lossy().to_string()))
                }
            },
            ImportMap::BlockAll => ImportMapBlockAll.resolve_import_path(path, parent)
        }
    }
    fn load_import(&self, id: String) -> anyhow::Result<String> {
        match self {
            ImportMap::AllowHttp => Err(anyhow::anyhow!("HTTP imports should be loaded via fetch")),
            ImportMap::AllowFolder(folder) => {
                let full_path = Path::new(&id).canonicalize()?;
                if !is_inside(&folder, &full_path) {
                    return Err(anyhow::anyhow!("Attempted to load import outside of allowed folder"));
                }
                let content = std::fs::read_to_string(full_path)?;
                Ok(content)
            },
            ImportMap::BlockAll => ImportMapBlockAll.load_import(id)
        }
    }
}

fn is_inside(parent: &Path, child: &Path) -> bool {
    let parent = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let child = match child.canonicalize() {
        Ok(c) => c,
        Err(_) => return false,
    };
    child.starts_with(parent)
}
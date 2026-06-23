use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use hyper::Uri;

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
    fn resolve_import_path(
        &self,
        _path: String,
        _parent: String,
    ) -> anyhow::Result<ResolvedModule> {
        Err(anyhow::anyhow!("Importing modules is blocked"))
    }
    fn load_import(&self, _id: String) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Importing modules is blocked"))
    }
}

pub enum StaticImportSource {
    Url(Uri),
    File(PathBuf),
}
impl StaticImportSource {
    pub fn parse_string(s: String, basedir: &Path) -> anyhow::Result<Self> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(StaticImportSource::Url(s.parse()?))
        } else {
            Ok(StaticImportSource::File(basedir.join(s)))
        }
    }
}

#[derive(Clone, Default)]
pub enum ImportMap {
    #[default]
    AllowHttp,
    BlockAll,
    StaticImportMap(Arc<HashMap<String, StaticImportSource>>),
}
impl CustomImportMap for ImportMap {
    fn resolve_import_path(&self, path: String, parent: String) -> anyhow::Result<ResolvedModule> {
        match self {
            ImportMap::AllowHttp => Ok(ResolvedModule::Url(path)),
            ImportMap::BlockAll => ImportMapBlockAll.resolve_import_path(path, parent),
            ImportMap::StaticImportMap(map) => match map.get(&path) {
                Some(StaticImportSource::Url(url)) => Ok(ResolvedModule::Url(url.to_string())),
                Some(StaticImportSource::File(_)) => Ok(ResolvedModule::Id(path)),
                None => Err(anyhow::anyhow!(
                    "Module {} not found in static import map",
                    path
                )),
            },
        }
    }
    fn load_import(&self, id: String) -> anyhow::Result<String> {
        match self {
            ImportMap::AllowHttp => Err(anyhow::anyhow!("HTTP imports should be loaded via fetch")),
            ImportMap::BlockAll => ImportMapBlockAll.load_import(id),
            ImportMap::StaticImportMap(map) => {
                if let Some(StaticImportSource::File(path)) = map.get(&id) {
                    let content = std::fs::read_to_string(path)?;
                    Ok(content)
                } else {
                    Err(anyhow::anyhow!("Module not found"))
                }
            }
        }
    }
}

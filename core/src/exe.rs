use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

#[expect(clippy::disallowed_methods)]
static CURRENT_EXE: LazyLock<Option<PathBuf>> = LazyLock::new(|| match std::env::current_exe() {
    Ok(p) => Some(p),
    Err(err) => {
        log::error!("current exe failed: {err:?}");
        None
    }
});

static CURRENT_EXE_METADATA: LazyLock<Option<std::fs::Metadata>> = LazyLock::new(|| {
    let exe = current_exe()?;
    match std::fs::metadata(exe) {
        Ok(metadata) => Some(metadata),
        Err(err) => {
            log::error!("current exe read metadata {exe:?} failed: {err:?}");
            None
        }
    }
});

pub fn current_exe() -> Option<&'static Path> {
    (*CURRENT_EXE).as_deref()
}

pub fn current_exe_metadata() -> Option<&'static std::fs::Metadata> {
    (*CURRENT_EXE_METADATA).as_ref()
}

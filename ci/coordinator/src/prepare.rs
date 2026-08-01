use rain_lang::{
    afs::{
        Dir, File,
        generated::{dir::GeneratedDir, entry::GeneratedFSEntry, file::GeneratedFile},
        path::SealedFilePath,
    },
    driver::{CreateAreaOptions, DriverTrait, FSTrait as _},
};

#[expect(clippy::unwrap_used)]
pub fn prepare_ci_run_area_tar_gz(
    download: Vec<u8>,
) -> (
    GeneratedDir,
    Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
) {
    let config = rain_core::config::Config::new();
    let driver = rain_core::driver::DriverImpl::new(config);
    let download_area = driver
        .create_area(&[], &CreateAreaOptions::default())
        .unwrap();
    let download_entry =
        GeneratedFSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
    std::fs::write(driver.resolve_fs_entry((&download_entry).into()), download).unwrap();
    let download = GeneratedFile::new_checked(&driver, download_entry).unwrap();
    let raw_tar = driver
        .extract_gzip(&File::Generated(download), "extract_temp.tar")
        .unwrap();
    let area = driver.extract_tar(&File::Generated(raw_tar)).unwrap();
    prepare_ci_run_git_lfs_entries(&driver, area)
}

#[expect(clippy::unwrap_used)]
pub fn prepare_ci_run_area_zip(
    download: bytes::Bytes,
) -> (
    GeneratedDir,
    Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
) {
    let config = rain_core::config::Config::new();
    let driver = rain_core::driver::DriverImpl::new(config);
    let download_area = driver
        .create_area(&[], &CreateAreaOptions::default())
        .unwrap();
    let download_entry =
        GeneratedFSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
    std::fs::write(driver.resolve_fs_entry((&download_entry).into()), download).unwrap();
    let download = GeneratedFile::new_checked(&driver, download_entry).unwrap();
    let area = driver.extract_zip(&File::Generated(download)).unwrap();
    prepare_ci_run_git_lfs_entries(&driver, area)
}

#[expect(clippy::unwrap_used)]
fn prepare_ci_run_git_lfs_entries(
    driver: &impl DriverTrait,
    area: rain_lang::afs::generated::area::GeneratedFSArea,
) -> (
    GeneratedDir,
    Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
) {
    let mut ls = std::fs::read_dir(
        driver.resolve_fs_entry(GeneratedDir::root(area.clone()).fsinner().into()),
    )
    .unwrap();
    let entry = ls.next().unwrap().unwrap();
    let download_dir_name = entry.file_name().into_string().unwrap();
    let download_dir_entry =
        GeneratedFSEntry::new(area, SealedFilePath::new(&download_dir_name).unwrap());
    let root = GeneratedDir::new_checked(driver, download_dir_entry).unwrap();
    let lfs_entries: Vec<_> = driver
        .glob(&Dir::Generated(root.clone()), "**/*")
        .unwrap()
        .into_iter()
        .filter_map(|entry| {
            let path = driver.resolve_fs_entry(entry.fsinner());
            let lfs_object = git_lfs_rs::object::Object::from_path(&path).ok()?;
            Some((path, lfs_object))
        })
        .collect();
    (root, lfs_entries)
}

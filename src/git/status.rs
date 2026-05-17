use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub status: Status,
}

pub fn collect_status(repo_path: &Path) -> Result<Vec<StatusEntry>> {
    use gix::bstr::ByteSlice as _;
    use gix::status::index_worktree::Item as IwItem;
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    let repo = gix::open(repo_path)?;
    let platform = repo.status(gix::progress::Discard)?;
    let mut entries = Vec::new();

    for item in platform.into_iter(Vec::<gix::bstr::BString>::new())? {
        let item = item?;
        match item {
            // Changes between HEAD tree and index (staged changes)
            gix::status::Item::TreeIndex(change) => {
                use gix::diff::index::Change as TreeChange;
                let (path, status) = match &change {
                    TreeChange::Addition { location, .. } => {
                        (location.as_bstr().to_path_lossy().into_owned(), Status::Added)
                    }
                    TreeChange::Deletion { location, .. } => {
                        (location.as_bstr().to_path_lossy().into_owned(), Status::Deleted)
                    }
                    TreeChange::Modification { location, .. } => {
                        (location.as_bstr().to_path_lossy().into_owned(), Status::Modified)
                    }
                    TreeChange::Rewrite {
                        location, copy, ..
                    } => {
                        let status = if *copy { Status::Added } else { Status::Renamed };
                        (location.as_bstr().to_path_lossy().into_owned(), status)
                    }
                };
                entries.push(StatusEntry { path, status });
            }
            // Changes between index and worktree (unstaged changes + untracked files)
            gix::status::Item::IndexWorktree(iw_item) => {
                match iw_item {
                    IwItem::Modification { rela_path, status, .. } => {
                        let mapped = match status {
                            EntryStatus::Change(Change::Removed) => Status::Deleted,
                            EntryStatus::Change(Change::Modification { .. })
                            | EntryStatus::Change(Change::SubmoduleModification(_))
                            | EntryStatus::Change(Change::Type { .. }) => Status::Modified,
                            EntryStatus::Conflict { .. } => Status::Modified,
                            EntryStatus::IntentToAdd => Status::Added,
                            EntryStatus::NeedsUpdate(_) => continue,
                        };
                        let path = rela_path.to_path_lossy().into_owned();
                        entries.push(StatusEntry { path, status: mapped });
                    }
                    IwItem::DirectoryContents { entry, .. } => {
                        if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                            let path = entry.rela_path.to_path_lossy().into_owned();
                            entries.push(StatusEntry {
                                path,
                                status: Status::Untracked,
                            });
                        }
                    }
                    IwItem::Rewrite { dirwalk_entry, copy, .. } => {
                        let status = if copy { Status::Added } else { Status::Renamed };
                        let path = dirwalk_entry.rela_path.to_path_lossy().into_owned();
                        entries.push(StatusEntry { path, status });
                    }
                }
            }
        }
    }

    Ok(entries)
}

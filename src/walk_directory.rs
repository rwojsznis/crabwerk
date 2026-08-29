use ignore::{WalkBuilder, WalkState};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

use super::{
    file_utils::build_glob_set, pack::Pack, raw_configuration::RawConfiguration,
};

const PACKAGE_YML: &str = "package.yml";

pub struct WalkDirectoryResult {
    pub included_files: HashSet<PathBuf>,
    pub included_packs: HashSet<Pack>,
    pub owning_package_yml_for_file: HashMap<PathBuf, PathBuf>,
}

struct WalkedFile {
    absolute_path: PathBuf,
    is_package_yml: bool,
    is_pack_root: bool,
    is_included: bool,
}

// Skip hidden paths to match packwerk's use of `Dir.glob`.
pub fn walk_directory(
    absolute_root: PathBuf,
    raw: &RawConfiguration,
) -> anyhow::Result<WalkDirectoryResult> {
    // The user's `exclude` decides everything else. `.git` is the one
    // directory that stays hardcoded: it holds no Ruby the tool can use, and
    // walking it only costs time.
    let mut all_excluded_globs: Vec<String> = vec![String::from(".git/**/*")];
    all_excluded_globs.extend(raw.exclude.to_owned());

    let excludes_set = Arc::new(build_glob_set(&all_excluded_globs)?);
    let includes_set = Arc::new(build_glob_set(&raw.include)?);
    let package_paths_set = Arc::new(build_glob_set(&raw.package_paths)?);
    let root = Arc::new(absolute_root.clone());

    let mut builder = WalkBuilder::new(&absolute_root);
    builder
        // Gitignored Ruby can still be autoloaded, so only `exclude` filters it.
        .standard_filters(false)
        .hidden(true)
        .follow_links(true);

    let filter_root = root.clone();
    let filter_excludes = excludes_set.clone();
    // Prune excluded directories, but keep excluded files in the stream so
    // that an excluded `package.yml` can still register its pack.
    builder.filter_entry(move |entry| {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            return true;
        }

        match entry.path().strip_prefix(filter_root.as_ref()) {
            Ok(relative_path) => !filter_excludes.is_match(relative_path),
            Err(_) => true,
        }
    });

    let (sender, receiver) = mpsc::channel::<WalkedFile>();

    builder.build_parallel().run(|| {
        let sender = sender.clone();
        let root = root.clone();
        let includes_set = includes_set.clone();
        let excludes_set = excludes_set.clone();
        let package_paths_set = package_paths_set.clone();

        Box::new(move |entry| {
            // Match packwerk by skipping unreadable entries and invalid links.
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };

            if entry.file_type().is_none_or(|file_type| file_type.is_dir()) {
                return WalkState::Continue;
            }

            let absolute_path = entry.into_path();
            let Ok(relative_path) = absolute_path.strip_prefix(root.as_ref())
            else {
                return WalkState::Continue;
            };

            let is_package_yml = absolute_path
                .file_name()
                .is_some_and(|name| name == PACKAGE_YML);
            let is_pack_root = is_package_yml
                && relative_path.parent().is_some_and(|parent| {
                    // The root pack is the catch-all, even when it does not
                    // match `package_paths`.
                    package_paths_set.is_match(parent)
                        || parent == Path::new("")
                });
            let is_included = includes_set.is_match(relative_path)
                && !excludes_set.is_match(relative_path);

            if is_package_yml || is_included {
                let _ = sender.send(WalkedFile {
                    absolute_path,
                    is_package_yml,
                    is_pack_root,
                    is_included,
                });
            }

            WalkState::Continue
        })
    });

    drop(sender);

    let walked_files: Vec<WalkedFile> = receiver.into_iter().collect();

    // Resolve owners after the parallel walk has found every pack.
    let pack_dirs: HashSet<PathBuf> = walked_files
        .iter()
        .filter(|walked_file| walked_file.is_package_yml)
        .filter_map(|walked_file| walked_file.absolute_path.parent())
        .map(Path::to_path_buf)
        .collect();

    let mut included_files: HashSet<PathBuf> = HashSet::new();
    let mut included_packs: HashSet<Pack> = HashSet::new();
    let mut owning_package_yml_for_file: HashMap<PathBuf, PathBuf> =
        HashMap::new();

    for walked_file in walked_files {
        if walked_file.is_pack_root {
            included_packs.insert(Pack::from_path(
                &walked_file.absolute_path,
                &absolute_root,
            )?);
        }

        if walked_file.is_included {
            let package_yml = owning_package_yml(
                &walked_file.absolute_path,
                &absolute_root,
                &pack_dirs,
            );

            included_files.insert(walked_file.absolute_path.clone());
            owning_package_yml_for_file
                .insert(walked_file.absolute_path, package_yml);
        }
    }

    Ok(WalkDirectoryResult {
        included_files,
        included_packs,
        owning_package_yml_for_file,
    })
}

fn owning_package_yml(
    absolute_path: &Path,
    absolute_root: &Path,
    pack_dirs: &HashSet<PathBuf>,
) -> PathBuf {
    let mut directory = absolute_path.parent();

    while let Some(current) = directory {
        if pack_dirs.contains(current) {
            return current.join(PACKAGE_YML);
        }

        if current == absolute_root {
            break;
        }

        directory = current.parent();
    }

    PathBuf::from(PACKAGE_YML)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use crate::{
        raw_configuration::RawConfiguration, walk_directory::walk_directory,
    };

    #[test]
    fn test_walk_directory() -> anyhow::Result<()> {
        let absolute_path = PathBuf::from("tests/fixtures/simple_app")
            .canonicalize()
            .expect("Could not canonicalize path");

        let raw_config = RawConfiguration {
            include: vec!["**/*".to_string()],
            ..RawConfiguration::default()
        };

        let walk_directory_result =
            walk_directory(absolute_path.clone(), &raw_config);
        assert!(walk_directory_result.is_ok());
        let included_files = walk_directory_result?.included_files;

        let node_module_file =
            absolute_path.join("node_modules/subfolder/file.rb");
        let contains_bad_file = included_files.contains(&node_module_file);
        assert!(!contains_bad_file);

        let node_module_file = absolute_path.join("node_modules/file.rb");
        let contains_bad_file = included_files.contains(&node_module_file);
        assert!(!contains_bad_file);

        Ok(())
    }

    #[test]
    fn test_walk_directory_owns_files_by_nearest_package_yml()
    -> anyhow::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;
        std::fs::create_dir_all(root.join("packs/foo/nested/app"))?;
        std::fs::write(root.join("package.yml"), "enforce_privacy: false")?;
        std::fs::write(
            root.join("packs/foo/package.yml"),
            "enforce_privacy: false",
        )?;
        std::fs::write(
            root.join("packs/foo/nested/package.yml"),
            "enforce_privacy: false",
        )?;
        std::fs::write(root.join("root.rb"), "")?;
        std::fs::write(root.join("packs/foo/foo.rb"), "")?;
        std::fs::write(root.join("packs/foo/nested/app/deep.rb"), "")?;

        let raw_config = RawConfiguration {
            include: vec!["**/*.rb".to_string()],
            exclude: vec![],
            ..RawConfiguration::default()
        };

        let owners = walk_directory(root.clone(), &raw_config)?
            .owning_package_yml_for_file;

        assert_eq!(
            owners.get(&root.join("root.rb")),
            Some(&root.join("package.yml"))
        );
        assert_eq!(
            owners.get(&root.join("packs/foo/foo.rb")),
            Some(&root.join("packs/foo/package.yml"))
        );
        assert_eq!(
            owners.get(&root.join("packs/foo/nested/app/deep.rb")),
            Some(&root.join("packs/foo/nested/package.yml"))
        );

        Ok(())
    }

    #[test]
    fn test_walk_directory_ignores_gitignore() -> anyhow::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;
        // A real `.git` directory, so that a walker which reads gitignore
        // files only inside a repository does read this one.
        std::fs::create_dir_all(root.join(".git"))?;
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n")?;
        std::fs::write(root.join(".gitignore"), "ignored.rb\nignored_dir/\n")?;
        std::fs::write(root.join("ignored.rb"), "")?;
        std::fs::create_dir_all(root.join("ignored_dir"))?;
        std::fs::write(root.join("ignored_dir/file.rb"), "")?;

        let raw_config = RawConfiguration {
            include: vec!["**/*.rb".to_string()],
            exclude: vec![],
            ..RawConfiguration::default()
        };

        let included_files =
            walk_directory(root.clone(), &raw_config)?.included_files;

        assert!(included_files.contains(&root.join("ignored.rb")));
        assert!(included_files.contains(&root.join("ignored_dir/file.rb")));

        Ok(())
    }

    #[test]
    fn test_walk_directory_skips_hidden_files() -> anyhow::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;
        std::fs::create_dir_all(root.join(".hidden_dir"))?;
        std::fs::write(root.join(".hidden_dir/file.rb"), "")?;
        std::fs::write(root.join(".hidden.rb"), "")?;
        std::fs::write(root.join("visible.rb"), "")?;

        let raw_config = RawConfiguration {
            include: vec!["**/*.rb".to_string()],
            exclude: vec![],
            ..RawConfiguration::default()
        };

        let included_files =
            walk_directory(root.clone(), &raw_config)?.included_files;

        assert!(included_files.contains(&root.join("visible.rb")));
        assert!(!included_files.contains(&root.join(".hidden_dir/file.rb")));
        assert!(!included_files.contains(&root.join(".hidden.rb")));

        Ok(())
    }

    #[test]
    fn test_walk_directory_finds_nested_packs() -> anyhow::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;
        std::fs::create_dir_all(root.join("packs/foo/nested"))?;
        std::fs::create_dir_all(root.join("excluded/pack"))?;
        std::fs::write(root.join("package.yml"), "enforce_privacy: false")?;
        std::fs::write(
            root.join("packs/foo/package.yml"),
            "enforce_privacy: false",
        )?;
        std::fs::write(
            root.join("packs/foo/nested/package.yml"),
            "enforce_privacy: false",
        )?;
        std::fs::write(
            root.join("excluded/pack/package.yml"),
            "enforce_privacy: false",
        )?;

        let raw_config = RawConfiguration {
            include: vec!["**/*.rb".to_string()],
            exclude: vec!["excluded/**/*".to_string()],
            ..RawConfiguration::default()
        };

        let pack_names: HashSet<String> = walk_directory(root, &raw_config)?
            .included_packs
            .into_iter()
            .map(|pack| pack.name)
            .collect();

        assert_eq!(
            pack_names,
            HashSet::from([
                String::from("."),
                String::from("packs/foo"),
                String::from("packs/foo/nested"),
            ])
        );

        Ok(())
    }

    #[test]
    fn test_walk_directory_always_skips_git() -> anyhow::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let absolute_root = temp_dir.path().canonicalize()?;
        std::fs::create_dir_all(absolute_root.join(".git/hooks"))?;
        std::fs::write(absolute_root.join(".git/config.rb"), "")?;
        std::fs::write(absolute_root.join(".git/hooks/hook.rb"), "")?;
        std::fs::write(absolute_root.join("app.rb"), "")?;

        let raw_config = RawConfiguration {
            exclude: vec![],
            ..RawConfiguration::default()
        };

        let included_files =
            walk_directory(absolute_root.clone(), &raw_config)?.included_files;

        assert!(included_files.contains(&absolute_root.join("app.rb")));
        assert!(
            !included_files.contains(&absolute_root.join(".git/config.rb"))
        );
        assert!(
            !included_files.contains(&absolute_root.join(".git/hooks/hook.rb"))
        );

        Ok(())
    }
}

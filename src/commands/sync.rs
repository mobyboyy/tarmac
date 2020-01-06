use std::{collections::VecDeque, env, fs, path::Path};

use sha2::{Digest, Sha256};
use snafu::ResultExt;

use crate::{
    auth_cookie::get_auth_cookie,
    data::{Config, Manifest},
    options::{GlobalOptions, SyncOptions, SyncTarget},
};

mod error {
    use crate::data::{ConfigError, ManifestError};
    use snafu::Snafu;
    use std::{io, path::PathBuf};

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub(super)")]
    pub enum SyncError {
        #[snafu(display("{}", source))]
        Config {
            source: ConfigError,
        },

        #[snafu(display("{}", source))]
        Manifest {
            source: ManifestError,
        },

        Io {
            path: PathBuf,
            source: io::Error,
        },

        #[snafu(display("couldn't get the current directory of the process"))]
        CurrentDir {
            source: io::Error,
        },

        #[snafu(display("'tarmac sync' requires an authentication cookie"))]
        NoAuth,
    }
}

pub use error::SyncError;

pub fn sync(global: GlobalOptions, options: SyncOptions) -> Result<(), SyncError> {
    let fuzzy_config_path = match options.config_path {
        Some(v) => v,
        None => env::current_dir().context(error::CurrentDir)?,
    };

    let mut session = SyncSession::new(&fuzzy_config_path)?;

    session.discover_configs()?;
    session.discover_inputs()?;

    match options.target {
        SyncTarget::Roblox => {
            let auth = global
                .auth
                .or_else(get_auth_cookie)
                .expect("no auth cookie found");

            // session.sync_to_roblox(auth)?;
        }
        SyncTarget::ContentFolder => {
            // session.sync_to_content_folder()?;
        }
    }

    // session.write_manifest()?;
    // session.codegen()?;

    Ok(())
}

/// A sync session holds all of the state for a single run of the 'tarmac sync'
/// command.
#[derive(Debug)]
struct SyncSession {
    /// The config file pulled from the starting point of the sync operation.
    root_config: Config,

    /// Config files discovered by searching through the `includes` section of
    /// known config files, recursively.
    non_root_configs: Vec<Config>,

    /// The manifest file that was present as of the beginning of the sync
    /// operation.
    original_manifest: Manifest,
}

impl SyncSession {
    fn new(fuzzy_config_path: &Path) -> Result<Self, SyncError> {
        log::trace!("Starting new sync session");

        let root_config =
            Config::read_from_folder_or_file(&fuzzy_config_path).context(error::Config)?;

        log::trace!("Starting from config \"{}\"", root_config.name);

        let original_manifest = match Manifest::read_from_folder(root_config.folder()) {
            Ok(manifest) => manifest,
            Err(err) if err.is_not_found() => Manifest::default(),
            other => other.context(error::Manifest)?,
        };

        Ok(Self {
            root_config,
            non_root_configs: Vec::new(),
            original_manifest,
        })
    }

    fn discover_configs(&mut self) -> Result<(), SyncError> {
        let mut to_search = VecDeque::new();
        to_search.extend(
            self.root_config
                .includes
                .iter()
                .map(|include| include.path.clone()),
        );

        while let Some(search_path) = to_search.pop_front() {
            let search_meta =
                fs::metadata(&search_path).context(error::Io { path: &search_path })?;

            if search_meta.is_file() {
                // This is a file that's explicitly named by a config. We'll
                // check that it's a Tarmac config and include it.

                let config = Config::read_from_file(&search_path).context(error::Config)?;
                to_search.extend(config.includes.iter().map(|include| include.path.clone()));
                self.non_root_configs.push(config);
            } else {
                // If this directory contains a config file, we can stop
                // traversing this branch.

                match Config::read_from_folder(&search_path) {
                    // We found a config, we're done here
                    Ok(config) => {
                        to_search
                            .extend(config.includes.iter().map(|include| include.path.clone()));
                        self.non_root_configs.push(config);
                    }

                    // We didn't find a config, keep searching
                    Err(err) if err.is_not_found() => {
                        let children =
                            fs::read_dir(&search_path).context(error::Io { path: &search_path })?;

                        for entry in children {
                            let entry = entry.context(error::Io { path: &search_path })?;
                            let entry_path = entry.path();

                            // DirEntry has a metadata method, but in the case
                            // of symlinks, it returns metadata about the
                            // symlink and not the file or folder.
                            let entry_meta = fs::metadata(&entry_path)
                                .context(error::Io { path: &entry_path })?;

                            if entry_meta.is_dir() {
                                to_search.push_back(entry_path);
                            }
                        }
                    }

                    // We hit some other error, cascade it upwards
                    err @ Err(_) => {
                        err.context(error::Config)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn discover_inputs(&mut self) -> Result<(), SyncError> {
        Ok(())
    }
}

fn is_image_asset(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        // TODO: Expand the definition of images?
        Some("png") | Some("jpg") => true,

        _ => false,
    }
}

fn generate_asset_hash(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

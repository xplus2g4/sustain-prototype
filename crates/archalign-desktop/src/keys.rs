//! API-key lookup (FR-SEC-04, ADR 0023): the provider's environment variable, then `.env`
//! in the launch working directory, then `.env` in the config directory. `.env` files are
//! parsed with `dotenvy::from_path_iter`; the process environment is never modified, and
//! the imported repository's `.env` is never read.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where a key came from (shown in Settings; the key itself never is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// The provider's environment variable.
    Environment,
    /// A `.env` file.
    DotEnv(PathBuf),
    /// No key was found.
    NotFound,
}

impl fmt::Display for KeySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeySource::Environment => f.write_str("environment"),
            KeySource::DotEnv(p) => write!(f, ".env at {}", p.display()),
            KeySource::NotFound => f.write_str("not found"),
        }
    }
}

/// An API key. `Debug` and `Display` never show it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// The secret, for the HTTP client only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

/// Reads environment variables (injectable for tests).
pub type EnvLookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Where to look.
#[derive(Clone)]
pub struct KeyEnv {
    /// Environment lookup (the process environment by default).
    pub vars: EnvLookup,
    /// The working directory the app was launched from.
    pub launch_dir: Option<PathBuf>,
    /// The app's config directory.
    pub config_dir: Option<PathBuf>,
}

impl fmt::Debug for KeyEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyEnv")
            .field("launch_dir", &self.launch_dir)
            .field("config_dir", &self.config_dir)
            .finish_non_exhaustive()
    }
}

impl KeyEnv {
    /// The real process environment, the current directory and the given config dir.
    pub fn process(config_dir: Option<PathBuf>) -> Self {
        Self {
            vars: Arc::new(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty())),
            launch_dir: std::env::current_dir().ok(),
            config_dir,
        }
    }
}

fn from_dotenv(path: &Path, var: &str) -> Option<String> {
    // Only regular files: a folder named `.env` (a Python virtualenv) or a FIFO would make
    // the reader return errors forever or block (NFR-QUAL-04).
    if !path.metadata().is_ok_and(|m| m.is_file()) {
        return None;
    }
    let iter = dotenvy::from_path_iter(path).ok()?;
    // Malformed lines are skipped; an I/O error ends the lookup.
    iter.take_while(|r| !matches!(r, Err(dotenvy::Error::Io(..))))
        .filter_map(Result::ok)
        .find(|(k, _)| k == var)
        .map(|(_, v)| v)
        .filter(|v| !v.trim().is_empty())
}

/// Resolves the key for an environment variable name (e.g. `OPENAI_API_KEY`).
pub fn resolve(env: &KeyEnv, var: &str) -> (KeySource, Option<ApiKey>) {
    if let Some(v) = (env.vars)(var) {
        return (KeySource::Environment, Some(ApiKey(v)));
    }
    for dir in [&env.launch_dir, &env.config_dir].into_iter().flatten() {
        let path = dir.join(".env");
        if let Some(v) = from_dotenv(&path, var) {
            return (KeySource::DotEnv(path), Some(ApiKey(v)));
        }
    }
    (KeySource::NotFound, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(
        vars: Vec<(&'static str, &'static str)>,
        launch: Option<&Path>,
        config: Option<&Path>,
    ) -> KeyEnv {
        KeyEnv {
            vars: Arc::new(move |k| {
                vars.iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }),
            launch_dir: launch.map(Path::to_path_buf),
            config_dir: config.map(Path::to_path_buf),
        }
    }

    #[test]
    fn lookup_order_is_environment_then_launch_dir_then_config_dir() {
        let launch = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        std::fs::write(launch.path().join(".env"), "OPENAI_API_KEY=from-launch\n").unwrap();
        std::fs::write(
            config.path().join(".env"),
            "OPENAI_API_KEY=from-config\nOTHER=x\n",
        )
        .unwrap();
        let (src, key) = resolve(
            &env(
                vec![("OPENAI_API_KEY", "from-env")],
                Some(launch.path()),
                Some(config.path()),
            ),
            "OPENAI_API_KEY",
        );
        assert_eq!(src, KeySource::Environment);
        assert_eq!(key.unwrap().expose(), "from-env");
        let (src, key) = resolve(
            &env(vec![], Some(launch.path()), Some(config.path())),
            "OPENAI_API_KEY",
        );
        assert_eq!(src, KeySource::DotEnv(launch.path().join(".env")));
        assert_eq!(key.unwrap().expose(), "from-launch");
        std::fs::write(launch.path().join(".env"), "SOMETHING_ELSE=1\n").unwrap();
        let (src, key) = resolve(
            &env(vec![], Some(launch.path()), Some(config.path())),
            "OPENAI_API_KEY",
        );
        assert_eq!(src, KeySource::DotEnv(config.path().join(".env")));
        assert_eq!(key.unwrap().expose(), "from-config");
    }

    #[test]
    fn not_found_when_no_source_has_the_key() {
        let empty = tempfile::tempdir().unwrap();
        let (src, key) = resolve(&env(vec![], Some(empty.path()), None), "OPENAI_API_KEY");
        assert_eq!(src, KeySource::NotFound);
        assert!(key.is_none());
        assert_eq!(src.to_string(), "not found");
    }

    #[test]
    fn the_key_is_redacted_in_debug_output() {
        let k = ApiKey("sk-secret-123".into());
        assert_eq!(format!("{k:?}"), "ApiKey(<redacted>)");
        assert!(!format!("{:?}", Some(k)).contains("sk-secret"));
    }

    /// A Python virtualenv named `.env` (a folder) used to make the lookup loop forever.
    #[test]
    fn a_folder_named_env_is_skipped() {
        let launch = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        std::fs::create_dir(launch.path().join(".env")).unwrap();
        std::fs::write(config.path().join(".env"), "OPENAI_API_KEY=from-config\n").unwrap();
        let (src, key) = resolve(
            &env(vec![], Some(launch.path()), Some(config.path())),
            "OPENAI_API_KEY",
        );
        assert_eq!(src, KeySource::DotEnv(config.path().join(".env")));
        assert_eq!(key.unwrap().expose(), "from-config");
    }
}

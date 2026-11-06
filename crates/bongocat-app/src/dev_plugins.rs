//! The source of available plugins is fixed by the build environment.
use bongocat_config::BuildEnvironment;
use bongocat_plugin_service::PluginSource;
use std::path::{Path, PathBuf};

/// Development-only directory containing locally built plugin packages.
const DEVELOPMENT_DIRECTORY_VARIABLE: &str = "BONGOCAT_PLUGIN_DEV_DIRECTORY";

pub(crate) fn source() -> PluginSource {
    match bongocat_app::BUILD_ENVIRONMENT {
        BuildEnvironment::Development => {
            let packages = development_plugin_directory();
            PluginSource::Development {
                catalog: packages.join("catalog.json"),
                packages,
            }
        }
        BuildEnvironment::Production => PluginSource::Production {
            catalog_url: bongocat_update::PLUGIN_CATALOG_URL.into(),
            public_key: bongocat_update::PLUGIN_RELEASE_SIGNING_KEY.into(),
            platform: bongocat_update::HOST_TARGET_TRIPLE
                .manifest_platform()
                .into(),
        },
    }
}

/// Resolve Development plugins in this order:
///
/// 1. An explicit developer override.
/// 2. The plugin directory bundled beside this installed application.
/// 3. The checkout-local directory used by `just dev`.
///
/// The third path is useful only on the build machine. Packaged Development
/// builds must use the second path so they do not depend on the CI runner's
/// checkout directory.
fn development_plugin_directory() -> PathBuf {
    if let Some(path) = std::env::var_os(DEVELOPMENT_DIRECTORY_VARIABLE)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        && path.join("catalog.json").is_file()
    {
        return path;
    }

    if let Ok(executable) = std::env::current_exe()
        && let Some(packages) = bundled_plugin_directory(&executable)
        && packages.join("catalog.json").is_file()
    {
        return packages;
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("target/plugin-dev/plugins")
}

/// The packaged resource root differs between Windows and macOS.
#[cfg(target_os = "windows")]
fn bundled_plugin_directory(executable: &Path) -> Option<PathBuf> {
    Some(executable.parent()?.join("resources/plugins"))
}

#[cfg(target_os = "macos")]
fn bundled_plugin_directory(executable: &Path) -> Option<PathBuf> {
    Some(executable.parent()?.parent()?.join("Resources/plugins"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn bundled_plugin_directory(_executable: &Path) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_fixed_and_environment_isolated() {
        match source() {
            PluginSource::Development { catalog, packages } => {
                assert!(catalog.ends_with(std::path::Path::new(
                    "target/plugin-dev/plugins/catalog.json"
                )) || catalog.ends_with(std::path::Path::new("resources/plugins/catalog.json")));
                assert!(
                    packages.ends_with("target/plugin-dev/plugins")
                        || packages.ends_with("resources/plugins")
                );
            }
            PluginSource::Production {
                catalog_url,
                platform,
                public_key,
            } => {
                assert_eq!(
                    catalog_url,
                    "https://raw.githubusercontent.com/ayangweb/BongoCat/plugin-catalog/plugins.json"
                );
                assert!(!platform.is_empty());
                assert!(!public_key.is_empty());
                assert_ne!(
                    public_key,
                    bongocat_update::RELEASE_SIGNING_KEY.unwrap_or_default()
                );
            }
            PluginSource::InstalledOnly => panic!("product must discover available plugins"),
        }
    }

    #[test]
    fn the_development_plugin_directory_is_an_explicit_build_input() {
        assert_eq!(
            DEVELOPMENT_DIRECTORY_VARIABLE,
            "BONGOCAT_PLUGIN_DEV_DIRECTORY"
        );
        assert!(!DEVELOPMENT_DIRECTORY_VARIABLE.starts_with("BONGOCAT_APP_"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packaged_windows_plugins_live_beside_the_installed_executable() {
        let executable = Path::new(r"C:\Program Files\Tukkiiie\bongocat-app.exe");
        assert_eq!(
            bundled_plugin_directory(executable).unwrap(),
            Path::new(r"C:\Program Files\Tukkiiie\resources\plugins")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn packaged_macos_plugins_live_in_contents_resources() {
        let executable = Path::new("/Applications/Tukkiiie.app/Contents/MacOS/bongocat-app");
        assert_eq!(
            bundled_plugin_directory(executable).unwrap(),
            Path::new("/Applications/Tukkiiie.app/Contents/Resources/plugins")
        );
    }
}

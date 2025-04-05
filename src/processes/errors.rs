#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ProcessError {
    SpawnCommandFailed(anyhow::Error),

    ProcessConfigMissingCommand,

    GetCurrentDirFailed(std::io::Error),
}

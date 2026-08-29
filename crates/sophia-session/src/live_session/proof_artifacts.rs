use super::*;

pub(super) struct LiveInputProofResult {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
}

pub(super) struct LiveClientStdoutCapture {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl LiveClientStdoutCapture {
    pub(super) fn create(
        display_number: u32,
    ) -> Result<(Self, std::fs::File), Box<dyn std::error::Error>> {
        let mut nonce = [0u8; 8];
        fill_session_random(&mut nonce)?;
        let suffix = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let directory = std::env::temp_dir().join(format!(
            "sophia-client-stdout-{}-{display_number}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let path = directory.join("stdout");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        Ok((Self { directory, path }, file))
    }

    pub(super) fn read_bounded(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)?
            .take(4_097)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

impl Drop for LiveClientStdoutCapture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

impl LiveInputProofResult {
    pub(super) fn create(display_number: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut nonce = [0u8; 8];
        fill_session_random(&mut nonce)?;
        let suffix = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let directory = std::env::temp_dir().join(format!(
            "sophia-input-proof-{}-{display_number}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let path = directory.join("received");
        Ok(Self { directory, path })
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(super) fn received(&self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for LiveInputProofResult {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

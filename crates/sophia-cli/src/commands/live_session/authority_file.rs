use super::*;

pub(super) struct LiveXAuthorityFile {
    path: Option<std::path::PathBuf>,
}

impl LiveXAuthorityFile {
    pub(super) fn create(
        display_number: u32,
    ) -> Result<(Self, [u8; 16]), Box<dyn std::error::Error>> {
        Self::create_in(&live_xauthority_directory(), display_number)
    }

    pub(super) fn create_in(
        directory: &std::path::Path,
        display_number: u32,
    ) -> Result<(Self, [u8; 16]), Box<dyn std::error::Error>> {
        let mut cookie = [0u8; 16];
        fill_session_random(&mut cookie)?;
        let mut nonce = [0u8; 8];
        fill_session_random(&mut nonce)?;
        let suffix = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = directory.join(format!(
            ".sophia-Xauthority-{}-{display_number}-{suffix}",
            std::process::id()
        ));
        let record = encode_live_xauthority_record(display_number, cookie)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let create_result = (|| -> Result<(), Box<dyn std::error::Error>> {
            file.write_all(&record)?;
            file.sync_all()?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(())
        })();
        if let Err(error) = create_result {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok((Self { path: Some(path) }, cookie))
    }

    pub(super) fn path(&self) -> &std::path::Path {
        self.path
            .as_deref()
            .expect("live Xauthority path is retained until cleanup")
    }

    pub(super) fn remove(&mut self) -> Result<(), std::io::Error> {
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for LiveXAuthorityFile {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

fn live_xauthority_directory() -> std::path::PathBuf {
    let effective_user = rustix::process::geteuid().as_raw();
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| {
                metadata.is_dir()
                    && metadata.uid() == effective_user
                    && metadata.permissions().mode() & 0o077 == 0
            })
        })
        .unwrap_or_else(std::env::temp_dir)
}

pub(super) fn fill_session_random(bytes: &mut [u8]) -> Result<(), std::io::Error> {
    let mut filled = 0;
    while filled < bytes.len() {
        let written =
            rustix::rand::getrandom(&mut bytes[filled..], rustix::rand::GetRandomFlags::empty())?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "kernel random source returned no bytes",
            ));
        }
        filled += written;
    }
    Ok(())
}

fn encode_live_xauthority_record(
    display_number: u32,
    cookie: [u8; 16],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const FAMILY_LOCAL: u16 = 256;
    let system = rustix::system::uname();
    let hostname = system.nodename().to_bytes();
    let display = display_number.to_string();
    let mut record = Vec::with_capacity(64 + hostname.len());
    record.extend_from_slice(&FAMILY_LOCAL.to_be_bytes());
    push_xauthority_field(&mut record, hostname)?;
    push_xauthority_field(&mut record, display.as_bytes())?;
    push_xauthority_field(&mut record, b"MIT-MAGIC-COOKIE-1")?;
    push_xauthority_field(&mut record, &cookie)?;
    Ok(record)
}

fn push_xauthority_field(
    output: &mut Vec<u8>,
    field: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let len = u16::try_from(field.len()).map_err(|_| "Xauthority field exceeds u16")?;
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(field);
    Ok(())
}

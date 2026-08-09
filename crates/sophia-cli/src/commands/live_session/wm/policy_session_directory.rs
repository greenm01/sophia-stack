struct PolicySessionDirectory {
    path: std::path::PathBuf,
}

impl PolicySessionDirectory {
    fn create(path: std::path::PathBuf) -> Result<Self, std::io::Error> {
        std::fs::create_dir(&path)?;
        if let Err(error) = std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(0o700),
        ) {
            let _ = std::fs::remove_dir(&path);
            return Err(error);
        }
        Ok(Self { path })
    }

    fn endpoint_path(&self) -> std::path::PathBuf {
        self.path.join("endpoint")
    }

    fn checkpoint_path(&self) -> std::path::PathBuf {
        self.path.join("hagia-policy.checkpoint")
    }
}

impl Drop for PolicySessionDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.checkpoint_path());
        let _ = std::fs::remove_dir(&self.path);
    }
}

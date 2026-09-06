use super::*;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

enum CatalogJob {
    Refresh(u64),
    Verify(u64, ApplicationCatalogEntry),
}
pub enum ApplicationCatalogWorkerResult {
    Built(u64, Result<ApplicationCatalog, String>),
    Verified(u64, Result<ApplicationLaunchCommand, String>),
    Unavailable,
}
pub struct ApplicationCatalogWorker {
    sender: SyncSender<CatalogJob>,
    receiver: Receiver<ApplicationCatalogWorkerResult>,
    busy: bool,
    disconnected: bool,
}
impl ApplicationCatalogWorker {
    pub fn start(
        config: sophia_config::ApplicationCatalogConfig,
        registered: Vec<RegisteredCatalogApplication>,
        environment: ApplicationCatalogEnvironment,
    ) -> std::io::Result<Self> {
        let (sender, jobs) = sync_channel(1);
        let (results, receiver) = sync_channel(1);
        std::thread::Builder::new()
            .name("sophia-catalog".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let snapshot = build_application_catalog(&config, &registered, &environment);
                    let result = match job {
                        CatalogJob::Refresh(id) => {
                            ApplicationCatalogWorkerResult::Built(id, snapshot)
                        }
                        CatalogJob::Verify(id, expected) => {
                            let command = snapshot.and_then(|catalog| {
                                let current = catalog.entries.iter().find(|entry| {
                                    entry.source == expected.source
                                        && entry.command == expected.command
                                        && entry.descriptor.label == expected.descriptor.label
                                        && entry.descriptor.available
                                });
                                revalidate_catalog_entry(
                                    current.ok_or("catalog changed; reopen launcher")?,
                                )
                            });
                            ApplicationCatalogWorkerResult::Verified(id, command)
                        }
                    };
                    if results.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender,
            receiver,
            busy: false,
            disconnected: false,
        })
    }
    pub fn refresh(&mut self, id: u64) -> bool {
        self.submit(CatalogJob::Refresh(id))
    }
    pub fn verify(&mut self, id: u64, entry: ApplicationCatalogEntry) -> bool {
        self.submit(CatalogJob::Verify(id, entry))
    }
    fn submit(&mut self, job: CatalogJob) -> bool {
        if self.busy || self.sender.try_send(job).is_err() {
            return false;
        }
        self.busy = true;
        true
    }
    pub fn poll(&mut self) -> Option<ApplicationCatalogWorkerResult> {
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if self.disconnected {
                    return None;
                }
                self.disconnected = true;
                ApplicationCatalogWorkerResult::Unavailable
            }
        };
        self.busy = false;
        Some(result)
    }
}

use std::sync::atomic::AtomicBool;

use crate::services::{ProcessManager, ProjectRepository};

pub struct AppState {
    pub repository: ProjectRepository,
    pub process_manager: ProcessManager,
    pub quitting: AtomicBool,
}

impl AppState {
    pub fn new(repository: ProjectRepository) -> Self {
        Self {
            repository,
            process_manager: ProcessManager::default(),
            quitting: AtomicBool::new(false),
        }
    }
}

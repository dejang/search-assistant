use super::{Engine, EngineStatus};

pub struct RemoteEngine;

impl Engine for RemoteEngine {
    fn endpoint(&self) -> &str {
        todo!()
    }

    fn status(&self) -> EngineStatus {
        todo!()
    }

    fn shutdown(&self) {
        todo!()
    }
}

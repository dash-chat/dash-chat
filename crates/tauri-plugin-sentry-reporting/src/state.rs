use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use crate::logs::PendingLogs;
use crate::transport::UserInitiatedTransport;
use crate::{client, Config};

pub(crate) type Sentry<'a> = State<'a, Arc<SentryState>>;

pub struct SentryState {
    /// A guard rather than a `Client` because dropping it at shutdown is the
    /// point: `close` flushes the transport queue. Derefs to the client.
    pub(crate) client: sentry::ClientInitGuard,
    pub(crate) data_dir: PathBuf,
    pub(crate) pending: Arc<PendingLogs>,
    pub(crate) transport: Arc<UserInitiatedTransport>,
}

impl SentryState {
    pub(crate) fn new(config: Config, transport: Arc<UserInitiatedTransport>) -> Arc<Self> {
        let pending = Arc::new(PendingLogs::default());
        Arc::new(Self {
            client: sentry::init(client::options(&config, pending.clone(), transport.clone())),
            data_dir: config.data_dir,
            pending,
            transport,
        })
    }
}

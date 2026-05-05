use named_id::RenameNone;
use serde::{Deserialize, Serialize};

comcap::capabilities! {
    #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, RenameNone)]
    pub struct Capabilities {
        messaging: 1,
    }
}

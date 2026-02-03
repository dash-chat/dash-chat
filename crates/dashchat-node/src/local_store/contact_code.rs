use redb::*;

use crate::ContactCode;

use super::LocalStore;

pub const CONTACT_CODE_TABLE: TableDefinition<&'static str, &str> =
    TableDefinition::new("contact_code");
const CONTACT_CODE_KEY: &str = "contact_code";

impl LocalStore {
    pub fn get_contact_code(&self) -> anyhow::Result<Option<ContactCode>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CONTACT_CODE_TABLE)?;
        match table.get(CONTACT_CODE_KEY)? {
            Some(value) => {
                let code_str = value.value();
                let code = code_str.parse::<ContactCode>()?;
                Ok(Some(code))
            }
            None => Ok(None),
        }
    }

    pub fn set_contact_code(&self, code: &ContactCode) -> anyhow::Result<()> {
        let code_str = code.to_string();
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CONTACT_CODE_TABLE)?;
            table.insert(CONTACT_CODE_KEY, code_str.as_str())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn clear_contact_code(&self) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CONTACT_CODE_TABLE)?;
            table.remove(CONTACT_CODE_KEY)?;
        }
        txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use chrono::{Duration, Utc};

    fn create_test_contact_code() -> ContactCode {
        use p2panda_core::PublicKey;
        use p2panda_spaces::ActorId;

        let pubkey = PublicKey::from_bytes(&[11; 32]).unwrap();
        let agent_id = AgentId::from(ActorId::from_bytes(&[22; 32]).unwrap());
        ContactCode {
            device_pubkey: DeviceId::from(pubkey),
            inbox_topic: Some(InboxTopic {
                topic: Topic::inbox(),
                expires_at: Utc::now() + Duration::hours(1),
            }),
            agent_id,
            share_intent: crate::ShareIntent::AddContact,
        }
    }

    #[test]
    fn test_get_contact_code_returns_none_when_not_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_get_contact_code_none.db");
        let store = LocalStore::new(&path).unwrap();

        let code = store.get_contact_code().unwrap();
        assert!(code.is_none());
    }

    #[test]
    fn test_set_and_get_contact_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_set_get_contact_code.db");
        let store = LocalStore::new(&path).unwrap();

        let code = create_test_contact_code();
        store.set_contact_code(&code).unwrap();

        let retrieved = store.get_contact_code().unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), code);
    }

    #[test]
    fn test_set_contact_code_overwrites_previous() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_set_contact_code_overwrite.db");
        let store = LocalStore::new(&path).unwrap();

        let code1 = create_test_contact_code();
        store.set_contact_code(&code1).unwrap();

        // Create a different code
        let mut code2 = create_test_contact_code();
        code2.inbox_topic = Some(InboxTopic {
            topic: Topic::new([99; 32]),
            expires_at: Utc::now() + Duration::hours(2),
        });

        store.set_contact_code(&code2).unwrap();

        let retrieved = store.get_contact_code().unwrap().unwrap();
        assert_eq!(retrieved, code2);
        assert_ne!(retrieved, code1);
    }

    #[test]
    fn test_clear_contact_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_clear_contact_code.db");
        let store = LocalStore::new(&path).unwrap();

        let code = create_test_contact_code();
        store.set_contact_code(&code).unwrap();

        // Verify it was set
        assert!(store.get_contact_code().unwrap().is_some());

        // Clear it
        store.clear_contact_code().unwrap();

        // Verify it's gone
        assert!(store.get_contact_code().unwrap().is_none());
    }

    #[test]
    fn test_clear_contact_code_when_not_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_clear_contact_code_not_set.db");
        let store = LocalStore::new(&path).unwrap();

        // Should not error when clearing a non-existent code
        store.clear_contact_code().unwrap();
        assert!(store.get_contact_code().unwrap().is_none());
    }

    #[test]
    fn test_contact_code_persists_across_store_instances() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_contact_code_persistence.db");

        let code = create_test_contact_code();

        // Set the code in one store instance
        {
            let store = LocalStore::new(&path).unwrap();
            store.set_contact_code(&code).unwrap();
        }

        // Read it back from a new store instance
        {
            let store = LocalStore::new(&path).unwrap();
            let retrieved = store.get_contact_code().unwrap().unwrap();
            assert_eq!(retrieved, code);
        }
    }
}

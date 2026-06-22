#![allow(unused)]

use anyhow::anyhow;
use std::time::Duration;

/// Install the rustls `ring` crypto provider. Required before any TLS handshake;
/// idempotent — `install_default()` returns `Err` on re-call, which we ignore.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Route panics through `log::error!` before the default hook runs. Release
/// builds use `panic = "abort"`, so a panic aborts before its message reaches
/// stderr/the unified log; logging it first preserves the panic location and
/// message in the on-device log file for crash investigations. Idempotent.
pub fn install_panic_hook() {
    static PANIC_HOOK_ONCE: std::sync::Once = std::sync::Once::new();
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|l| l.to_string())
                .unwrap_or_else(|| "unknown location".to_string());
            log::error!("panic at {location}: {info}");
            default_hook(info);
        }));
    });
}

pub async fn with_retries<T>(
    condition: impl AsyncFn() -> anyhow::Result<T>,
    retries: usize,
    sleep_ms: u64,
) -> anyhow::Result<T> {
    let mut retry_count = 0;
    loop {
        let response = condition().await;

        match response {
            Ok(r) => {
                return Ok(r);
            }
            Err(err) => {
                log::warn!("Condition not met yet: {err:?} Retrying in {}ms.", sleep_ms);
                std::thread::sleep(Duration::from_millis(sleep_ms));

                retry_count += 1;
                if retry_count == retries {
                    return Err(anyhow!("Timeout. Last error: {err:?}"));
                }
            }
        }
    }
}

// pub async fn migrate_app(
//     holochain_runtime: &HolochainRuntime,
//     existing_app_id: InstalledAppId,
//     new_app_id: InstalledAppId,
//     new_app_bundle: AppBundle,
//     new_roles_settings: Option<RoleSettingsMap>,
// ) -> anyhow::Result<AppInfo> {
//     log::info!(
//         "Migrating from old app {} to new app {}.",
//         existing_app_id,
//         new_app_id
//     );
//     let admin_ws = holochain_runtime.admin_websocket().await?;
//     let apps = admin_ws.list_apps(None).await?;

//     let Some(existing_app_info) = apps
//         .into_iter()
//         .find(|app| app.installed_app_id.eq(&existing_app_id))
//     else {
//         return Err(anyhow!("Existing app {} not found.", existing_app_id));
//     };

//     let mut new_roles_settings = new_roles_settings.unwrap_or_default();

//     let mut roles_settings = RoleSettingsMap::new();

//     let mut migrated_roles_from_cells: HashMap<RoleName, CellId> = HashMap::new();

//     // For every new role
//     // - Check if there was an existing provisioned cell
//     //   - If there wasn't, use given roles settings
//     //   - If there was:
//     //     - Compute new dna and compare with existing
//     //       - If the dna has not changed, add the RolesSettings::UseExisting
//     //       - If the dna has changed, use given roles settings
//     for new_role in new_app_bundle.manifest().app_roles() {
//         let new_role_settings = new_roles_settings.remove(&new_role.name);

//         if let Some(new_role_settings) = &new_role_settings {
//             if let RoleSettings::UseExisting { cell_id } = new_role_settings {
//                 roles_settings.insert(
//                     new_role.name,
//                     RoleSettings::UseExisting {
//                         cell_id: cell_id.clone(),
//                     },
//                 );
//                 continue;
//             }
//         };

//         let existing_cells = existing_app_info.cell_info.get(&new_role.name);

//         let Some(existing_cell) =
//             existing_cells
//                 .cloned()
//                 .unwrap_or_default()
//                 .iter()
//                 .find_map(|c| match c {
//                     CellInfo::Provisioned(c) => Some(c.clone()),
//                     _ => None,
//                 })
//         else {
//             if let Some(role_settings) = new_role_settings {
//                 roles_settings.insert(new_role.name, role_settings);
//             }
//             continue;
//         };

//         let new_modifiers = match &new_role_settings {
//             Some(RoleSettings::Provisioned { modifiers, .. }) => match modifiers {
//                 Some(modifiers) => match modifiers.properties.clone() {
//                     Some(properties) => {
//                         let bytes = SerializedBytes::try_from(properties)?;
//                         Some(DnaModifiersOpt {
//                             network_seed: modifiers.network_seed.clone(),
//                             properties: Some(bytes),
//                         })
//                     }
//                     None => None,
//                 },
//                 None => None,
//             },
//             _ => None,
//         };

//         let Some(new_dna_hash) =
//             dna_hash_for_app_bundle_role(&new_app_bundle, &new_role.name, new_modifiers).await?
//         else {
//             return Err(anyhow!("Invalid new dna hash."));
//         };

//         if new_dna_hash.eq(&existing_cell.cell_id.dna_hash()) {
//             log::info!("Reusing role {}.", new_role.name);

//             roles_settings.insert(
//                 new_role.name,
//                 RoleSettings::UseExisting {
//                     cell_id: existing_cell.cell_id,
//                 },
//             );
//         } else {
//             if let Some(role_settings) = new_role_settings {
//                 roles_settings.insert(new_role.name.clone(), role_settings);
//             };
//             migrated_roles_from_cells.insert(new_role.name, existing_cell.cell_id);
//         }
//     }

//     let roles_settings = if roles_settings.is_empty() {
//         None
//     } else {
//         Some(roles_settings)
//     };

//     let app_info = holochain_runtime
//         .install_app(
//             new_app_id,
//             new_app_bundle,
//             roles_settings,
//             Some(existing_app_info.agent_pub_key),
//             None,
//         )
//         .await?;

//     let app_ws = holochain_runtime
//         .app_websocket(app_info.installed_app_id.clone(), AllowedOrigins::Any)
//         .await?;

//     for (migrated_role, old_cell_id) in migrated_roles_from_cells {
//         let Some(CellInfo::Provisioned(provisioned_cell)) = app_info
//             .cell_info
//             .get(&migrated_role)
//             .cloned()
//             .unwrap_or_default()
//             .first()
//             .cloned()
//         else {
//             log::info!(
//                 "Role {migrated_role} was marked for migration but was not created upon app installation."
//             );
//             continue;
//         };

//         let zomes = find_zomes_with_zome_trait(
//             &admin_ws,
//             &app_ws,
//             &provisioned_cell.cell_id,
//             migration_zome_trait::MIGRATION_ZOME_TRAIT_HASH,
//         )
//         .await?;

//         for zome in zomes {
//             log::debug!("Migrating zome {zome} in role {migrated_role}...");
//             app_ws
//                 .call_zome(
//                     ZomeCallTarget::CellId(provisioned_cell.cell_id.clone()),
//                     zome.clone(),
//                     "migrate".into(),
//                     ExternIO::encode(old_cell_id.clone())?,
//                 )
//                 .await?;
//             log::info!("Successfully migrated zome {zome} in role {migrated_role}.");
//         }
//     }

//     Ok(app_info)
// }

// pub async fn dna_hash_for_app_bundle_role(
//     app_bundle: &AppBundle,
//     role_name: &RoleName,
//     dna_modifiers: Option<DnaModifiersOpt>,
// ) -> anyhow::Result<Option<DnaHash>> {
//     let Some(role) = app_bundle
//         .manifest()
//         .app_roles()
//         .into_iter()
//         .find(|r| r.name.eq(role_name))
//     else {
//         return Ok(None);
//     };

//     let Some(DnaLocation::Bundled(path)) = role.dna.location else {
//         return Ok(None);
//     };

//     let Some(dna_bundle_bytes) = app_bundle.bundled_resources().get(&path) else {
//         return Ok(None);
//     };

//     let bundle = DnaBundle::decode(dna_bundle_bytes.inner())?;

//     let (dna_file, _) = bundle.into_dna_file(DnaModifiersOpt::default()).await?;

//     let dna_def = dna_file.dna_def().clone();

//     let dna_def = if let Some(modifiers) = dna_modifiers {
//         dna_def.update_modifiers(modifiers)
//     } else {
//         dna_def
//     };

//     Ok(Some(DnaHash::with_data_sync(&dna_def)))
// }

// pub async fn find_zomes_with_zome_trait(
//     admin_ws: &AdminWebsocket,
//     app_ws: &AppWebsocket,
//     cell_id: &CellId,
//     zome_trait_hash: [u8; 32],
// ) -> anyhow::Result<Vec<ZomeName>> {
//     let dna_def = admin_ws
//         .get_dna_definition(cell_id.dna_hash().clone())
//         .await?;

//     let mut zomes = vec![];

//     for (coordinator_zome, _) in dna_def.coordinator_zomes {
//         let traits = get_implemented_traits(
//             app_ws,
//             ZomeCallTarget::CellId(cell_id.clone()),
//             coordinator_zome.clone(),
//         )
//         .await?;

//         if traits.iter().any(|t| t.eq(&zome_trait_hash)) {
//             zomes.push(coordinator_zome);
//         }
//     }

//     Ok(zomes)
// }

// pub async fn get_implemented_traits(
//     app_ws: &AppWebsocket,
//     cell: ZomeCallTarget,
//     zome_name: ZomeName,
// ) -> anyhow::Result<Vec<[u8; 32]>> {
//     let Ok(response) = app_ws
//         .call_zome(
//             cell,
//             zome_name,
//             "__implemented_zome_traits".into(),
//             ExternIO::encode(()).unwrap(),
//         )
//         .await
//     else {
//         return Ok(vec![]);
//     };
//     let implemented_zome_traits: Vec<[u8; 32]> = response.decode()?;

//     Ok(implemented_zome_traits)
// }

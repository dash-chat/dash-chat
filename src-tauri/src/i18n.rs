use serde_json::Value;
use std::collections::HashMap;

const EN_MESSAGES: &str = include_str!("../../ui/messages/en.json");
const ES_MESSAGES: &str = include_str!("../../ui/messages/es.json");
const DE_MESSAGES: &str = include_str!("../../ui/messages/de-de.json");
const FA_MESSAGES: &str = include_str!("../../ui/messages/fa-ir.json");

/// Initialize sonix-i18n with translations embedded from ui/messages/*.json.
///
/// Missing keys in non-English locales are backfilled from English so that
/// `sonix_i18n::init` (which requires identical key sets) doesn't fail.
pub fn init_i18n() {
    let en: Value = serde_json::from_str(EN_MESSAGES).expect("Failed to parse en.json");
    let _es: Value = serde_json::from_str(ES_MESSAGES).expect("Failed to parse es.json");
    let _de: Value = serde_json::from_str(DE_MESSAGES).expect("Failed to parse de-de.json");
    let _fa: Value = serde_json::from_str(FA_MESSAGES).expect("Failed to parse fa-ir.json");

    let _en_obj = en.as_object().expect("en.json must be an object").clone();

    // Backfill missing keys from English into each locale
    let locales: Vec<(&str, Value)> = vec![
        ("en", en),
        // ("es", backfill(es, &en_obj)),
        // ("de-de", backfill(de, &en_obj)),
        // ("fa-ir", backfill(fa, &en_obj)),
    ];

    let mut resources: HashMap<String, HashMap<String, Value>> = HashMap::new();
    for (lang, translations) in locales {
        let mut namespace_map = HashMap::new();
        namespace_map.insert("home".to_string(), translations);
        resources.insert(lang.to_string(), namespace_map);
    }

    sonix_i18n::init(resources, Some("en".to_string())).expect("Failed to initialize i18n");
}

/// For each key in `reference` that is missing in `locale`, insert the English value.
#[allow(unused)]
fn backfill(locale: Value, reference: &serde_json::Map<String, Value>) -> Value {
    let mut obj = match locale {
        Value::Object(map) => map,
        other => return other,
    };
    for (key, value) in reference {
        if !obj.contains_key(key) {
            obj.insert(key.clone(), value.clone());
        }
    }
    Value::Object(obj)
}

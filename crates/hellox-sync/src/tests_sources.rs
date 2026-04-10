use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    CachedManagedSettingsSource, CachedPolicyLimitsSource, EmptyManagedSettingsSource,
    EmptyPolicyLimitsSource, FileManagedSettingsSource, FilePolicyLimitsSource,
    ManagedSettingsDocument, ManagedSettingsSource, PolicyLimitsDocument, PolicyLimitsSource,
    RemoteDocument,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-sync-sources-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

#[test]
fn empty_sources_return_none() {
    assert!(EmptyManagedSettingsSource
        .load_managed_settings()
        .expect("load managed settings")
        .is_none());
    assert!(EmptyPolicyLimitsSource
        .load_policy_limits()
        .expect("load policy limits")
        .is_none());
}

#[test]
fn file_sources_load_local_documents() {
    let root = temp_dir();
    let managed_path = root.join("managed-settings.json");
    let policy_path = root.join("policy-limits.json");

    let managed = ManagedSettingsDocument {
        updated_at: 10,
        config_toml: "[permissions]\nmode = \"accept_edits\"\n".to_string(),
        signature: Some("sig-123".to_string()),
    };
    let policy = PolicyLimitsDocument {
        updated_at: 20,
        disabled_commands: vec!["plugin".to_string()],
        disabled_tools: vec!["bash".to_string()],
        notes: Some("local-only".to_string()),
    };

    fs::write(
        &managed_path,
        serde_json::to_string_pretty(&managed).expect("serialize managed"),
    )
    .expect("write managed file");
    fs::write(
        &policy_path,
        serde_json::to_string_pretty(&policy).expect("serialize policy"),
    )
    .expect("write policy file");

    assert_eq!(
        FileManagedSettingsSource::new(&managed_path)
            .load_managed_settings()
            .expect("read managed"),
        Some(managed)
    );
    assert_eq!(
        FilePolicyLimitsSource::new(&policy_path)
            .load_policy_limits()
            .expect("read policy"),
        Some(policy)
    );
}

#[test]
fn cached_sources_roundtrip_remote_documents() {
    let root = temp_dir();
    let managed_path = root.join("managed-cache.json");
    let policy_path = root.join("policy-cache.json");

    let managed_source = CachedManagedSettingsSource::with_path("dev", &managed_path);
    let policy_source = CachedPolicyLimitsSource::with_path("dev", &policy_path);

    let managed_document = RemoteDocument {
        etag: Some("etag-managed".to_string()),
        value: ManagedSettingsDocument {
            updated_at: 30,
            config_toml: "[tools]\nallow = [\"read\"]\n".to_string(),
            signature: None,
        },
    };
    let policy_document = RemoteDocument {
        etag: Some("etag-policy".to_string()),
        value: PolicyLimitsDocument {
            updated_at: 40,
            disabled_commands: vec!["server".to_string()],
            disabled_tools: vec!["shell".to_string()],
            notes: Some("cached".to_string()),
        },
    };

    managed_source
        .persist(&managed_document)
        .expect("persist managed cache");
    policy_source
        .persist(&policy_document)
        .expect("persist policy cache");

    let cached_managed = managed_source
        .inspect()
        .expect("inspect managed cache")
        .expect("managed cache");
    assert_eq!(cached_managed.etag.as_deref(), Some("etag-managed"));
    assert_eq!(
        managed_source
            .load_managed_settings()
            .expect("load managed cache"),
        Some(managed_document.value)
    );

    let cached_policy = policy_source
        .inspect()
        .expect("inspect policy cache")
        .expect("policy cache");
    assert_eq!(cached_policy.etag.as_deref(), Some("etag-policy"));
    assert_eq!(
        policy_source
            .load_policy_limits()
            .expect("load policy cache"),
        Some(policy_document.value)
    );
}

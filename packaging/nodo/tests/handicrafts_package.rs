//! The handicrafts feed app's files, checked with the same code the device uses.
//!
//! This file lives in the handicrafts repository (packaging/nodo/tests/) and runs inside a
//! monokulo-nodo checkout, where nodo's own manifest and settings parsers are: copy it to
//! crates/nodo-install/tests/ and set HANDICRAFTS_PACKAGING to handicrafts' packaging/nodo.
//! The feed workflow and scripts/test-nodo-package.sh do both.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use nodo_install::{Permission, Permissions};
use nodo_manifest::{Manifest, Outbound};
use nodo_settings::{load_settings, Installed, InstalledPackage, Limits, SecretString, Value};

fn packaging() -> PathBuf {
    let dir = std::env::var_os("HANDICRAFTS_PACKAGING").expect("HANDICRAFTS_PACKAGING must name handicrafts' packaging/nodo");
    PathBuf::from(dir)
}

fn files() -> PathBuf {
    packaging().join("srcpkgs/handicrafts/files")
}

fn manifest() -> Manifest {
    let text = std::fs::read_to_string(files().join("manifest.toml")).unwrap();
    Manifest::parse(&text.replace("@VERSION@", "0.2.2.32")).unwrap()
}

#[test]
fn the_manifest_passes_feed_validation_and_names_its_files() {
    let m = manifest();
    assert_eq!(m.id, "handicrafts");
    assert_eq!(m.exec, ["bin/handicrafts", "--listen", "127.0.0.1:8082"]);
    assert!(files().join("launcher").exists());
    assert!(m.needs.is_empty());
    assert!(m.storage.encrypted, "orders hold customers' names, addresses and emails");
    assert_eq!(m.network.outbound, Outbound::Internet, "Monokulo and the mail server are on the internet");
    assert_eq!(m.web.len(), 1);
    assert_eq!(m.web[0].upstream, "127.0.0.1:8082", "the launcher's --listen and the site agree");
    assert!(m.web[0].admin_paths.is_empty(), "/admin is public, as on the router, behind its own auth");
    assert_eq!(m.web[0].suggested_subdomain.as_deref(), Some("handicrafts"), "the local address is handicrafts.local");
    assert!(m.web[0].multi_address, "pages use relative links, so local and public at once");
    let p = Permissions::of(&m, &[]);
    assert!(p.granted.contains(&Permission::OutboundInternet));
    assert!(!p.granted.contains(&Permission::UnencryptedStorage));
}

#[test]
fn the_recipe_builds_offline_and_installs_the_launcher() {
    let recipe = std::fs::read_to_string(packaging().join("srcpkgs/handicrafts/template")).unwrap();
    assert!(recipe.contains("pkgname=handicrafts\n"));
    for placeholder in ["version=@VERSION@\n", "_commit=@COMMIT@\n", "checksum=\"@SOURCE_SHA256@\n"] {
        assert!(recipe.contains(placeholder), "scripts/nodo-dist.sh fills {placeholder:?}");
    }
    assert!(recipe.contains("--locked --offline"));
    assert!(recipe.contains("CARGO_NET_OFFLINE=true"));
    assert!(recipe.contains("\"${FILESDIR}/launcher\" \"${DESTDIR}/${_app}/bin/handicrafts\""));
    assert!(recipe.contains("_app=opt/nodo/apps/handicrafts\n"));
    let feed = std::fs::read_to_string(packaging().join("feed-packages")).unwrap();
    assert!(feed.lines().any(|l| l == "handicrafts"), "the feed publishes it");
}

#[test]
fn settings_load_and_resolve_with_defaults_only() {
    let source = std::fs::read_to_string(files().join("settings.lua")).unwrap();
    let loaded = load_settings(&source, Limits::default()).unwrap();
    let schema = loaded.schema().clone();
    for secret in ["admin_password_hash", "tuta_password", "smtp_password", "monokulo_secret_key", "monokulo_webhook_secret"]
    {
        assert_eq!(schema.setting(secret).unwrap().ty.name(), "secret", "{secret}");
        assert!(schema.setting(secret).unwrap().public.is_none(), "{secret} is never exported");
    }
    let mut installed = Installed::default();
    installed.packages.insert("handicrafts".into(), InstalledPackage { manifest: manifest(), schema, values: BTreeMap::new() });
    let resolved = installed.resolve("handicrafts").unwrap();
    assert_eq!(resolved.exec[1..], ["--listen", "127.0.0.1:8082"]);
    assert_eq!(resolved.values["public_url"], Value::Url("https://www.rachelshandicrafts.com".into()));
    assert_eq!(resolved.values["mail"], Value::Enum("off".into()));
    assert_eq!(resolved.values["limit_ip_10min"], Value::Int(3));
    assert_eq!(resolved.values["limit_shop_day"], Value::Int(40));
    assert!(!resolved.values.contains_key("admin_password_hash"), "/admin stays closed until it is set");
    assert!(resolved.dependencies.is_empty() && resolved.network.is_empty());

    let with = |name: &str, value: Value| {
        let mut values = resolved.values.clone();
        values.insert(name.into(), value);
        loaded.validate_values(&values)
    };
    assert!(!with("public_url", Value::Url("https://shop.example.org/shop".into())).is_empty());
    assert!(!with("admin_email", Value::String("rachel".into())).is_empty());
    assert!(!with("admin_password_hash", Value::Secret(SecretString::new("hunter2"))).is_empty(), "under 8 characters");
    assert!(with("admin_password_hash", Value::Secret(SecretString::new("correct horse"))).is_empty(), "a password");
    assert!(!with("monokulo_secret_key", Value::Secret(SecretString::new("pk_wrong"))).is_empty());
    assert!(!with("mail", Value::Enum("sendmail".into())).is_empty());
    assert!(!with("bsb", Value::String("12345".into())).is_empty());
    assert!(!with("limit_ip_day", Value::Int(0)).is_empty());
    assert!(with("public_url", Value::Url("https://shop.example.org".into())).is_empty());
    assert!(with("admin_email", Value::String("rachel@example.org".into())).is_empty());
    assert!(with(
        "admin_password_hash",
        Value::Secret(SecretString::new("$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNo"))
    )
    .is_empty());
    assert!(with("bsb", Value::String("062-000".into())).is_empty());
}

/// Runs the launcher with a stand-in for the shop that prints its arguments and environment.
fn launch(args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let (bin, libexec, home) = (dir.path().join("bin"), dir.path().join("libexec"), dir.path().join("home"));
    for d in [&bin, &libexec, &home] {
        std::fs::create_dir(d).unwrap();
    }
    std::fs::copy(files().join("launcher"), bin.join("handicrafts")).unwrap();
    let stand_in = libexec.join("handicrafts");
    std::fs::write(
        &stand_in,
        "#!/bin/sh\n\
         if [ \"${1:-}\" = hash-password ]; then\n\
           read -r p; [ ${#p} -ge 8 ] || { echo 'the password must be at least 8 characters' >&2; exit 1; }\n\
           echo \"\\$argon2id\\$stub\\$$p\"; exit 0\n\
         fi\n\
         for a do echo \"arg:$a\"; done\nenv | grep -E '^(HANDICRAFTS_|XDG_CACHE_HOME=)' | sort\n\
         echo \"nodo:$(env | grep -c '^NODO_')\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&stand_in, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = Command::new("sh");
    command.arg(bin.join("handicrafts")).args(args);
    command.env_clear().env("PATH", "/usr/bin:/bin").env("HOME", &home);
    for (k, v) in env {
        command.env(k, v);
    }
    let out = command.output().unwrap();
    let home = home.to_str().unwrap();
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).replace(home, "$HOME"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

const LISTEN: [&str; 2] = ["--listen", "127.0.0.1:8082"];

#[test]
fn the_launcher_maps_settings_and_keeps_the_data_in_home() {
    assert_eq!(
        launch(
            &LISTEN,
            &[
                ("NODO_PUBLIC_URL", "https://www.rachelshandicrafts.com"),
                ("NODO_MAIL", "tuta"),
                ("NODO_TUTA_PASSWORD", "s3cret pass"),
                ("NODO_LIMIT_IP_10MIN", "3"),
                // Stale variables from anywhere else never reach the shop.
                ("HANDICRAFTS_DATA_DIR", "/tmp"),
                ("HANDICRAFTS_ADMIN_PASSWORD_HASH", "$argon2id$stale"),
                ("NODO_UNRELATED", "x"),
            ]
        )
        .unwrap(),
        "HANDICRAFTS_DATA_DIR=$HOME\n\
         HANDICRAFTS_LIMIT_IP_10MIN=3\n\
         HANDICRAFTS_LISTEN=127.0.0.1:8082\n\
         HANDICRAFTS_MAIL=tuta\n\
         HANDICRAFTS_PUBLIC_URL=https://www.rachelshandicrafts.com\n\
         HANDICRAFTS_TRUSTED_PROXIES=127.0.0.1\n\
         HANDICRAFTS_TUTA_PASSWORD=s3cret pass\n\
         XDG_CACHE_HOME=$HOME/cache\n\
         nodo:0\n"
    );
}

#[test]
fn a_monokulo_public_key_alone_is_left_out() {
    let alone = launch(&LISTEN, &[("NODO_MONOKULO_PUBLIC_KEY", "pk_1")]).unwrap();
    assert!(!alone.contains("MONOKULO_PUBLIC_KEY"), "{alone}");
    let pasted = launch(
        &LISTEN,
        &[
            ("NODO_MONOKULO_PUBLIC_KEY", "pk_1"),
            ("NODO_MONOKULO_SECRET_KEY", "sk_1"),
            ("NODO_MONOKULO_WEBHOOK_SECRET", "whs"),
        ],
    )
    .unwrap();
    for line in ["HANDICRAFTS_MONOKULO_PUBLIC_KEY=pk_1", "HANDICRAFTS_MONOKULO_SECRET_KEY=sk_1", "HANDICRAFTS_MONOKULO_WEBHOOK_SECRET=whs"] {
        assert!(pasted.lines().any(|l| l == line), "{line} in {pasted}");
    }
}

#[test]
fn the_launcher_refuses_what_the_manifest_never_passes() {
    assert!(launch(&[], &[]).unwrap_err().contains("no --listen"));
    assert!(launch(&["--listen"], &[]).unwrap_err().contains("needs an address"));
    assert!(launch(&["--listen", "127.0.0.1:8082", "serve"], &[]).unwrap_err().contains("unexpected argument"));
    assert!(launch(&["--listen=127.0.0.1:9000"], &[]).unwrap().contains("HANDICRAFTS_LISTEN=127.0.0.1:9000\n"));
}

#[test]
fn the_launcher_hashes_a_password_and_passes_a_hash_through() {
    let hashed = launch(&LISTEN, &[("NODO_ADMIN_PASSWORD_HASH", "correct horse")]).unwrap();
    assert!(hashed.lines().any(|l| l == "HANDICRAFTS_ADMIN_PASSWORD_HASH=$argon2id$stub$correct horse"), "{hashed}");
    let from_router = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA";
    let kept = launch(&LISTEN, &[("NODO_ADMIN_PASSWORD_HASH", from_router)]).unwrap();
    assert!(kept.lines().any(|l| l == format!("HANDICRAFTS_ADMIN_PASSWORD_HASH={from_router}")), "{kept}");
    let none = launch(&LISTEN, &[]).unwrap();
    assert!(!none.contains("ADMIN_PASSWORD_HASH"), "/admin stays closed: {none}");
    let short = launch(&LISTEN, &[("NODO_ADMIN_PASSWORD_HASH", "short")]).unwrap_err();
    assert!(short.contains("could not be hashed"), "{short}");
}

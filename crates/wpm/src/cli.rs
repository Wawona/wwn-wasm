//! Shared CLI entry used by `bin/wpm` and `wpm_main`.

use anyhow::{bail, Context, Result};

use crate::store::PackageStore;
use crate::{DEFAULT_REGISTRY, REGISTRY_ENV, STORE_ENV};

struct Flags {
    notify: bool,
    update: bool,
    upgrade: bool,
}

fn parse_args<'a>(rest: &[&'a str]) -> Result<(String, Flags, Vec<&'a str>)> {
    let mut flags = Flags {
        notify: false,
        update: false,
        upgrade: false,
    };
    let mut positional = Vec::new();
    let mut skip_value = false;
    for arg in rest {
        if skip_value {
            skip_value = false;
            continue;
        }
        match *arg {
            "--notify" => flags.notify = true,
            "--update" => flags.update = true,
            "--upgrade" => flags.upgrade = true,
            "--all" => {}
            "--name" | "--version" => skip_value = true,
            "-h" | "--help" | "help" => positional.push("help"),
            s if s.starts_with('-') => bail!("unknown flag: {s} (try: wpm help)"),
            s => positional.push(s),
        }
    }
    if flags.update && flags.upgrade {
        bail!("pass only one of --update or --upgrade");
    }
    let cmd = if flags.update {
        "update"
    } else if flags.upgrade {
        "upgrade"
    } else {
        positional.first().copied().unwrap_or("help")
    };
    if flags.update || flags.upgrade {
        positional.retain(|s| *s != "update" && *s != "upgrade" && *s != "help");
    } else if !positional.is_empty() {
        positional.remove(0);
    }
    if flags.notify && cmd != "update" {
        bail!("--notify is only valid with update");
    }
    Ok((cmd.to_string(), flags, positional))
}

pub fn cli_run(args: &[String]) -> Result<i32> {
    let argv0 = args.first().map(|s| s.as_str()).unwrap_or("wpm");
    let rest: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
    let (cmd, flags, positional) = parse_args(&rest)?;
    let cmd = cmd.as_str();

    match cmd {
        "help" | "-h" | "--help" => {
            print_help(argv0);
            Ok(0)
        }
        "list" => {
            let store = PackageStore::open_default()?;
            let mut pkgs = store.list()?;
            pkgs.sort_by(|a, b| a.name.cmp(&b.name));
            if pkgs.is_empty() {
                println!("(no packages installed)");
            } else {
                println!("{:<24} {:<12} {:<8} {}", "NAME", "VERSION", "SOURCE", "DIGEST");
                for p in pkgs {
                    let dig = p.digest.get(..19).unwrap_or(&p.digest);
                    println!(
                        "{:<24} {:<12} {:<8} {}…",
                        p.name, p.version, p.source, dig
                    );
                }
            }
            Ok(0)
        }
        "path" => {
            let name = positional.first().context("usage: wpm path <name>")?;
            let store = PackageStore::open_default()?;
            println!("{}", store.resolve_wasm(name)?.display());
            Ok(0)
        }
        "remove" | "uninstall" => {
            let name = positional.first().context("usage: wpm remove <name>")?;
            let store = PackageStore::open_default()?;
            store.remove(name)?;
            println!("removed {name}");
            Ok(0)
        }
        "update" => cmd_update(flags.notify),
        "upgrade" => cmd_upgrade(positional.first().copied()),
        "install" => {
            let target = positional.first().copied().context("usage: wpm install <name|./file.wasm>")?;
            let store = PackageStore::open_default()?;
            if target.contains('/') || target.ends_with(".wasm") || target.starts_with('.') {
                let path = std::path::Path::new(target);
                let name = rest.iter().position(|a| *a == "--name").and_then(|i| rest.get(i + 1));
                let ver = rest
                    .iter()
                    .position(|a| *a == "--version")
                    .and_then(|i| rest.get(i + 1))
                    .copied()
                    .unwrap_or("0.0.0");
                let pkg = store.install_local(path, name.copied(), ver)?;
                println!(
                    "installed {} {} ({})",
                    pkg.name, pkg.version, pkg.digest
                );
                println!("run: wasm {}   or   wasm $(wpm path {})", pkg.name, pkg.name);
                Ok(0)
            } else {
                #[cfg(feature = "registry")]
                {
                    let (name, ver) = split_name_ver(target);
                    let client = crate::registry::RegistryClient::from_env_or_default()?;
                    client.install(&store, name, ver)?;
                    println!("installed {target} from {}", client.base());
                    println!("run: wasm {name}");
                    Ok(0)
                }
                #[cfg(not(feature = "registry"))]
                {
                    bail!("registry support not built; use: wpm install ./file.wasm");
                }
            }
        }
        "search" => {
            #[cfg(feature = "registry")]
            {
                let q = positional.first().copied().unwrap_or("");
                let client = crate::registry::RegistryClient::from_env_or_default()?;
                let hits = client.search(q)?;
                if hits.is_empty() {
                    println!("(no matches - is {DEFAULT_REGISTRY}/index.json published?)");
                } else {
                    println!("{:<24} {:<12} {}", "NAME", "VERSION", "SUMMARY");
                    for p in hits {
                        println!(
                            "{:<24} {:<12} {}",
                            p.name,
                            p.version,
                            p.summary.unwrap_or_default()
                        );
                    }
                }
                Ok(0)
            }
            #[cfg(not(feature = "registry"))]
            {
                bail!("registry support not built");
            }
        }
        "show" => {
            let name = positional.first().copied().context("usage: wpm show <name>")?;
            let store = PackageStore::open_default()?;
            if let Some(p) = store.get(name)? {
                println!("name:    {}", p.name);
                println!("version: {}", p.version);
                println!("digest:  {}", p.digest);
                println!("wasi:    {}", p.wasi);
                println!("source:  {}", p.source);
                println!("path:    {}", store.resolve_wasm(name)?.display());
                return Ok(0);
            }
            #[cfg(feature = "registry")]
            {
                let client = crate::registry::RegistryClient::from_env_or_default()?;
                let hits = client.search(name)?;
                let hit = hits.into_iter().find(|p| p.name == name);
                if let Some(p) = hit {
                    println!("name:    {} (registry)", p.name);
                    println!("version: {}", p.version);
                    println!("digest:  {}", p.digest);
                    println!("url:     {}/{}", client.base(), p.url);
                    if let Some(s) = p.summary {
                        println!("summary: {s}");
                    }
                    return Ok(0);
                }
            }
            bail!("package not found: {name}");
        }
        other => {
            bail!("unknown command: {other} (try: wpm help)");
        }
    }
}

#[cfg(feature = "registry")]
fn split_name_ver(spec: &str) -> (&str, Option<&str>) {
    if let Some((n, v)) = spec.split_once('@') {
        (n, Some(v))
    } else {
        (spec, None)
    }
}

fn cmd_update(notify: bool) -> Result<i32> {
    #[cfg(feature = "registry")]
    {
        use crate::registry::{upgrades_for, RegistryClient};
        let client = if notify {
            RegistryClient::from_env_for_notify()
        } else {
            RegistryClient::from_env_or_default()
        };
        let client = match client {
            Ok(client) => client,
            Err(err) if notify => {
                let _ = err;
                return Ok(0);
            }
            Err(err) => return Err(err),
        };
        let store = PackageStore::open_default()?;
        let installed = store.list()?;
        let index = match client.fetch_index() {
            Ok(index) => index,
            Err(err) if notify => {
                let _ = err;
                return Ok(0);
            }
            Err(err) => return Err(err),
        };
        let ups = upgrades_for(&installed, &index.packages);
        let actionable: Vec<_> = ups.iter().filter(|u| !u.local_sideload).collect();
        if notify {
            if actionable.is_empty() {
                return Ok(0);
            }
            println!(
                "wpm: {} package{} can be upgraded. Run `wpm upgrade`.",
                actionable.len(),
                if actionable.len() == 1 { "" } else { "s" }
            );
            for u in actionable {
                println!("  {} {} -> {}", u.name, u.from_version, u.to_version);
            }
            return Ok(0);
        }
        if ups.is_empty() {
            println!("wpm: all installed packages are current");
            return Ok(0);
        }
        println!("{:<24} {:<12} {:<12} {}", "NAME", "INSTALLED", "AVAILABLE", "NOTE");
        for u in &ups {
            let note = if u.local_sideload {
                "local sideload"
            } else {
                "registry"
            };
            println!(
                "{:<24} {:<12} {:<12} {note}",
                u.name, u.from_version, u.to_version
            );
        }
        if actionable.is_empty() {
            println!("wpm: no registry packages to upgrade (sideloads stay until `wpm upgrade <name>`)");
        } else {
            println!(
                "wpm: {} registry package{} can be upgraded. Run `wpm upgrade`.",
                actionable.len(),
                if actionable.len() == 1 { "" } else { "s" }
            );
        }
        Ok(0)
    }
    #[cfg(not(feature = "registry"))]
    {
        let _ = notify;
        bail!("registry support not built");
    }
}

fn cmd_upgrade(name: Option<&str>) -> Result<i32> {
    #[cfg(feature = "registry")]
    {
        use crate::registry::{upgrades_for, RegistryClient};
        let client = RegistryClient::from_env_or_default()?;
        let store = PackageStore::open_default()?;
        let installed = store.list()?;
        if let Some(name) = name {
            if store.get(name)?.is_none() {
                bail!("package not installed: {name} (try: wpm install {name})");
            }
        }
        let index = client.fetch_index()?;
        let ups = upgrades_for(&installed, &index.packages);
        let selected: Vec<_> = if let Some(name) = name {
            ups.into_iter().filter(|u| u.name == name).collect()
        } else {
            ups.into_iter().filter(|u| !u.local_sideload).collect()
        };
        if let Some(name) = name {
            if selected.is_empty() {
                if !index.packages.iter().any(|p| p.name == name) {
                    bail!("package not found in registry: {name}");
                }
                println!("wpm: {name} is current");
                return Ok(0);
            }
        } else if selected.is_empty() {
            println!("wpm: all installed packages are current");
            return Ok(0);
        }
        let n = selected.len();
        for u in selected {
            client.install(&store, &u.name, Some(&u.to_version))?;
            println!("upgraded {} {} -> {}", u.name, u.from_version, u.to_version);
        }
        println!(
            "wpm: upgraded {n} package{}",
            if n == 1 { "" } else { "s" }
        );
        Ok(0)
    }
    #[cfg(not(feature = "registry"))]
    {
        let _ = name;
        bail!("registry support not built");
    }
}

fn print_help(argv0: &str) {
    println!(
        "wpm - Wawona Runtime package manager (WASI .wasm)\n\
         \n\
         Usage: {argv0} <command>\n\
         \n\
           list                         Installed packages\n\
           install ./file.wasm          Register a local .wasm (Files.app / sideload)\n\
           install <name>[@ver]         Install from Mode A registry\n\
           remove <name>                Uninstall\n\
           path <name>                  Print blob path\n\
           show <name>                  Metadata\n\
           search [query]               Search {DEFAULT_REGISTRY}\n\
           update | --update            List registry upgrades\n\
           update --notify              Print only when upgrades exist (PTY login)\n\
           upgrade | --upgrade          Upgrade every registry package\n\
           upgrade <name>               Upgrade one package (includes sideload)\n\
           help                         This text\n\
         \n\
         Store:  ${STORE_ENV} or ~/Library/Application Support/Wawona/wasm-packages\n\
         Registry (Mode A only): ${REGISTRY_ENV} default {DEFAULT_REGISTRY}\n\
         Jailbreak .deb APT is a different channel - never used by wpm.\n\
         Run packages:  wasm <name>   (Runtime resolves installed names)\n"
    );
}

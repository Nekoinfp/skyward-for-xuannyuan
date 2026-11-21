use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use skyward_config::{ramdisk, DeviceConfig};
use skyward_sys::{bcb, Power};
use skyward_vab::SnapshotState;
use std::path::{Path, PathBuf};

#[cfg(target_os = "android")]
mod aod;
mod art;
// The two pieces of opening the phone's secure services that are not the// phone's own: what a hardware description says, and copying the libraries// the services load. Both are tried on the host, and both are used only where// the recovery service is built.
#[cfg(any(target_os = "android", test))]
mod framework;
#[cfg(any(target_os = "android", test))]
mod libraries;
#[cfg(target_os = "android")]
mod backup;
#[cfg(target_os = "android")]
mod decrypt;
#[cfg(target_os = "android")]
mod filework;
#[cfg(any(target_os = "android", test))]
mod find;
#[cfg(target_os = "android")]
mod snapuserd;
#[cfg(target_os = "android")]
mod haptics;
#[cfg(target_os = "android")]
mod input;
#[cfg(target_os = "android")]
mod merge;
#[cfg(target_os = "android")]
mod partwork;
#[cfg(target_os = "android")]
mod pty;
#[cfg(target_os = "android")]
mod readings;
#[cfg(target_os = "android")]
mod remembered;
#[cfg(target_os = "android")]
mod relabel;
#[cfg(target_os = "android")]
mod secure;
#[cfg(target_os = "android")]
mod services;
#[cfg(target_os = "android")]
mod settings;
#[cfg(target_os = "android")]
mod systems;
#[cfg(target_os = "android")]
mod ui;
#[cfg(target_os = "android")]
mod updates;
#[cfg(target_os = "android")]
mod vbmeta;
#[cfg(target_os = "android")]
mod wipe;
#[cfg(target_os = "android")]
mod wlan;

#[derive(Parser)]
#[command(name = "skyward", version, about = "Skyward recovery")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print what the About page shows, plus a partition summary.
    Info {
        /// Device configuration, e.g. devices/haotian.toml.
        #[arg(long)]
        config: String,
    },
    /// Print the suffix of the slot the phone runs from and, where the
    /// phone's boot control runs, what it holds of each slot.
    Slot,
    /// List modules in dependency load order from a source directory.
    Modules {
        #[arg(long, default_value = "/vendor/lib/modules")]
        source: String,
    },
    /// Load kernel modules and their dependencies from the configured
    /// sources; with no names, the configuration's own list.
    Load {
        #[arg(long, default_value = "/skyward/device.toml")]
        config: String,
        names: Vec<String>,
    },
    /// Run as the recovery service (what init starts as /system/bin/recovery).
    Recovery,
    /// Reboot through init: system, recovery, fastboot, bootloader, edl, or
    /// off.
    Reboot { target: String },
    /// Print the bootloader control block in misc.
    Bcb,
    /// Print the logical partition metadata in super.
    Lp {
        /// The super partition, or an image of its start.
        #[arg(long = "super", default_value = "/dev/block/by-name/super")]
        super_dev: PathBuf,
        /// Metadata slot: 0 for _a, 1 for _b. Defaults to the booted slot.
        #[arg(long)]
        slot: Option<u32>,
        /// Plan growing a logical partition, as `name=bytes`, and say what it
        /// would take. Nothing is written: the plan is what is shown.
        #[arg(long)]
        plan_grow: Option<String>,
    },
    /// Make a device-mapper device whose blocks are served from user space,
    /// look for the control device the kernel makes for it, and take it away
    /// again. Nothing is read from it and nothing is written: this only says
    /// whether this kernel serves a compressed snapshot through `dm-user`.
    #[cfg(target_os = "android")]
    DmUser {
        /// The name the kernel's misc device is made under, as the
        /// snapuserd that would serve it is told.
        name: String,
        /// How long the device is, in sectors. Nothing reads it here.
        #[arg(long, default_value_t = 8)]
        sectors: u64,
        /// The parameters as the target takes them, when the shape this
        /// module writes is not the one the kernel accepts.
        #[arg(long)]
        params: Option<String>,
    },
    /// Start the phone's own snapshot daemon out of the installed system, ask
    /// it whether it is serving, and stop it again. Nothing is mounted and
    /// nothing is written.
    #[cfg(target_os = "android")]
    Snapuserd {
        /// Begin merging what it serves, and watch how far each partition
        /// comes while it does.
        #[arg(long)]
        merge: bool,
    },
    /// Tell what a package or image is, the way the installer reads it.
    Identify { file: PathBuf },
    /// Discard all a block device holds and write zeros over its start, as
    /// a factory reset erases the data partition.
    #[cfg(target_os = "android")]
    Discard { device: PathBuf },
    /// Make a filesystem on a block device, f2fs or ext4, as a factory
    /// reset makes /metadata afresh.
    #[cfg(target_os = "android")]
    Mkfs { device: PathBuf, filesystem: String },
    /// Map the booted slot's partitions from super and mount them read-only.
    Mount,
    /// Unmount what `mount` mounted and remove its device-mapper devices.
    Umount,
    /// Print where an update stands: its state, the slot it came from, each
    /// snapshot as libsnapshot keeps it and as the kernel maps it, and the
    /// kernel's snapshot targets.
    Update,
    /// Merge the update into the slot the phone runs from, starting the
    /// merge where it has not begun, and follow it to its end.
    #[cfg(target_os = "android")]
    Merge {
        /// Merge an update whose snapshots are compressed, which only the
        /// daemon that serves them can do.
        #[arg(long)]
        compressed: bool,
    },
    /// Keep the configuration's services that this phone runs going until
    /// stopped.
    #[cfg(target_os = "android")]
    Services {
        #[arg(long, default_value = "/skyward/device.toml")]
        config: String,
    },
    /// List the display connectors, their modes, CRTCs and planes.
    #[cfg(target_os = "android")]
    Kms {
        #[arg(long, default_value = "/dev/dri/card0")]
        card: String,
    },
    /// Play the vibration motor's click once, or the ticks of the screen
    /// going off by the power key, saying how strongly and for how long.
    #[cfg(target_os = "android")]
    Haptic {
        #[arg(long, default_value = "/skyward/device.toml")]
        config: String,
        /// Where on the strength slider, from 0 at its left end to 1 at its
        /// right.
        #[arg(long, default_value_t = skyward_ui::app::SLIDER_START)]
        position: f32,
        /// Play the ticks of the screen going off instead of the click.
        #[arg(long)]
        ticks: bool,
    },
    /// Save what the panel shows, as raw RGBA files cap-000.rgba onwards.
    #[cfg(target_os = "android")]
    Capture {
        out: PathBuf,
        #[arg(long, default_value_t = 1)]
        count: u32,
        /// Pause between captures, in milliseconds.
        #[arg(long, default_value_t = 100)]
        interval: u64,
        #[arg(long, default_value = "/dev/dri/card0")]
        card: String,
    },
    /// Set the system clock from the RTC and the offset Android keeps on
    /// persist, as the recovery does when it starts, and print the time in
    /// China, the first of the time zones the settings offer.
    #[cfg(target_os = "android")]
    Clock,
    /// Say whether a service has registered with the phone's service
    /// directory, such as android.hardware.security.keymint.IKeyMintDevice/default.
    #[cfg(target_os = "android")]
    Service { name: String },
    /// Report the phone's key service and, given a key directory Android
    /// wrote, unwrap the key it holds and say how long it is.
    #[cfg(target_os = "android")]
    Keymint {
        #[arg(long, default_value = "/metadata/vold/metadata_encryption/key")]
        key: PathBuf,
        /// Report instead what the key service keeps under this name, such
        /// as profile_key_name_decrypt_999.
        #[arg(long)]
        alias: Option<String>,
    },
    /// Say what the phone's credential service answers, which takes no
    /// credential and changes nothing.
    #[cfg(target_os = "android")]
    Gatekeeper,
    /// Say what the phone's weaver says of itself, which reads no slot and
    /// changes nothing.
    #[cfg(target_os = "android")]
    Weaver,
    /// Open the data partition with the key the phone keeps for it, check
    /// it and mount it writable as the phone does, and open each user's
    /// files that open; the owner's shared storage then shows at /sdcard.
    #[cfg(target_os = "android")]
    Data {
        /// What a user would type at their screen lock, where one is set.
        /// A wrong one is counted against that user by the phone's secure
        /// world, the same as a wrong try on the screen itself.
        #[arg(long)]
        credential: Option<String>,
        /// Close the data partition instead, as a factory reset does before
        /// it erases it: every other program holding it ends first.
        #[arg(long, conflicts_with = "credential")]
        close: bool,
    },
    /// Say what opening the data partition needs, and whether each part is in
    /// place: the phone's own table, the key it keeps, the services the key
    /// chain runs, and what each of those services said on its own error
    /// output. Nothing is written and nothing is changed.
    #[cfg(target_os = "android")]
    Check {
        #[arg(long, default_value = "/skyward/device.toml")]
        config: PathBuf,
    },
    /// Drive the panel and touchscreen with the recovery UI.
    #[cfg(target_os = "android")]
    Ui {
        #[arg(long, default_value = "/skyward/device.toml")]
        config: std::path::PathBuf,
        #[arg(long, default_value = "/skyward/MiSansVF.ttf")]
        font: std::path::PathBuf,
        /// The terminal's monospaced typeface.
        #[arg(long, default_value = "/skyward/KirkMono-Regular.ttf")]
        mono: std::path::PathBuf,
        /// Open on the unlock page instead of the home page.
        #[arg(long)]
        unlock: bool,
        /// After each frame, also write it as raw RGBA to this file.
        #[arg(long)]
        dump: Option<std::path::PathBuf>,
    },
}

fn main() -> Result<()> {
    // init starts the recovery service by path with no arguments.
    let started_as = std::env::args_os().next().unwrap_or_default();
    let name = Path::new(&started_as).file_name().unwrap_or_default();
    if name == "recovery" {
        // A panic in a build that aborts leaves nothing behind: the process
        // is gone and the log says only that it went. What it was doing and
        // where it was doing it go to the kernel log first, which the
        // computer can read afterwards - a recovery that dies while opening
        // the phone's files is otherwise impossible to ask why.
        std::panic::set_hook(Box::new(|info| {
            let where_ = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "an unknown place".to_string());
            let what = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
            let _ = skyward_sys::kmsg(&format!("skyward panicked at {where_}: {what}"));
        }));
        return recovery();
    }
    // Install scripts run the commands linked to this binary by their names.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(code) = skyward_install::commands::run(name, &args) {
        std::process::exit(code);
    }
    match Cli::parse().cmd {
        Cmd::Info { config } => info(&config),
        Cmd::Slot => slot(),
        Cmd::Modules { source } => modules(&source),
        Cmd::Load { config, names } => {
            let cfg = DeviceConfig::load(&config)?;
            let names = if names.is_empty() { cfg.modules.load.clone() } else { names };
            for path in load_modules(&cfg, &names)? {
                println!("{}", path.display());
            }
            Ok(())
        }
        Cmd::Recovery => recovery(),
        Cmd::Reboot { target } => {
            let p = Power::parse(&target).ok_or_else(|| anyhow!("unknown target {target:?}"))?;
            skyward_sys::power(p)
        }
        Cmd::Bcb => {
            let b = bcb::read(&bcb::misc())?;
            println!("command = {:?}", b.command);
            println!("args    = {:?}", b.args);
            println!("request = {:?}", b.request());
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Discard { device } => skyward_block::erase::discard(&device),
        #[cfg(target_os = "android")]
        Cmd::Mkfs { device, filesystem } => skyward_block::erase::make(&device, &filesystem, &[], false),
        Cmd::Lp { super_dev, slot, plan_grow } => lp(&super_dev, slot, plan_grow.as_deref()),
        #[cfg(target_os = "android")]
        Cmd::DmUser { name, sectors, params } => dm_user(&name, sectors, params.as_deref()),
        #[cfg(target_os = "android")]
        Cmd::Snapuserd { merge } => if merge { snapuserd::diagnose(true) } else { snapuserd::probe() },
        Cmd::Update => update(),
        #[cfg(target_os = "android")]
        Cmd::Merge { compressed } => {
            let slot = booted_slot()?;
            // A compressed snapshot merges in the daemon that serves it, and
            // starts there too: `Waiting` is the kernel's kind, `Compressed`
            // the daemon's, and both have to be begun rather than only asked
            // after.
            let start = matches!(
                skyward_vab::standing(slot)?,
                skyward_vab::Standing::Waiting | skyward_vab::Standing::Compressed
            );
            let end = if compressed {
                // An update whose snapshots are compressed is merged by the
                // daemon that serves them, never by the kernel.
                merge::run_served(slot, &mut |m| println!("{m:?}"), &mut |name, said, share| {
                    println!("  {name:24} {said}  ({:.0}% of the snapshots done)", share * 100.0);
                })?
            } else {
                merge::run(slot, start, &mut |m| println!("{m:?}"))?
            };
            println!("ended {end:?}");
            Ok(())
        }
        Cmd::Identify { file } => identify(&file),
        Cmd::Mount => {
            let mounts = skyward_vab::mount_partitions(booted_slot()?)?;
            for d in &mounts.damaged {
                println!("damaged: {d}");
            }
            for m in &mounts.partitions {
                println!("{:<14} {:<16} {:<18} {}", m.partition, m.device.display(), m.mount_point.display(), m.filesystem);
            }
            Ok(())
        }
        Cmd::Umount => skyward_vab::unmount_partitions(booted_slot()?),
        #[cfg(target_os = "android")]
        Cmd::Services { config } => {
            let props = skyward_info::installed_props(booted_slot()?)?;
            let mut services = DeviceConfig::load(&config)?.services;
            services.retain(|s| s.runs_on(&props));
            services::start(&services, secure::libraries()?.as_deref())?;
            loop {
                std::thread::park();
            }
        }
        #[cfg(target_os = "android")]
        Cmd::Kms { card } => {
            print!("{}", skyward_ui::surface::kms::report(&card)?);
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Haptic { config, position, ticks } => {
            let haptics = DeviceConfig::load(&config)?.haptics.context("the configuration describes no vibration motor")?;
            haptics::probe(&haptics, position, ticks)
        }
        #[cfg(target_os = "android")]
        Cmd::Capture { out, count, interval, card } => {
            std::fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
            for i in 0..count {
                let t0 = std::time::Instant::now();
                let c = skyward_ui::surface::kms::capture(&card)?;
                let path = out.join(format!("cap-{i:03}.rgba"));
                std::fs::write(&path, &c.data).with_context(|| format!("write {}", path.display()))?;
                println!("{} {}x{} in {} ms", path.display(), c.w, c.h, t0.elapsed().as_millis());
                std::thread::sleep(std::time::Duration::from_millis(interval));
            }
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Clock => {
            let ms = skyward_sys::clock::set_from_rtc()?;
            println!("{}", skyward_ui::fmt::datetime(skyward_ui::Lang::En, (ms / 1000) as i64, 0));
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Service { name } => {
            let binder = skyward_binder::driver::Binder::open()?;
            let described = if skyward_binder::service::declared(&binder, &name)? { "described" } else { "not described" };
            let running = match skyward_binder::service::find(&binder, &name)? {
                Some(_) => "registered",
                None => "not registered",
            };
            println!("{name} is {described} and {running}");
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Keymint { key, alias } => {
            let binder = skyward_binder::driver::Binder::open()?;
            let service = skyward_crypto::keymint::KeyMint::open(&binder)?;
            let hw = service.hardware()?;
            println!("keymint {} version {} at security level {} by {}", hw.name, hw.version, hw.security_level, hw.author);
            if let Some(alias) = alias {
                let held = skyward_crypto::keystore2::key_blob(Path::new(skyward_crypto::keystore2::DATABASE), &alias)?;
                println!("{alias} is a key of {} bytes", held.len());
                for (tag, value) in service.authorizations(&held, &[])? {
                    println!("  tag {tag} = {value} ({value:#x})");
                }
                match service.usable(&held, &[]) {
                    Ok(()) => println!("the key service unwraps it and will use it"),
                    Err(e) => println!("the key service will not use it: {e:#}"),
                }
                return Ok(());
            }
            let stored = skyward_crypto::keystore::StoredKey::load(&key)?;
            for (tag, value) in stored.authorizations(&service)? {
                println!("  tag {tag} = {value}");
            }
            let raw = stored.unwrap(skyward_crypto::keystore::Unwrap::Hardware(&service))?;
            println!("{} holds a key of {} bytes", key.display(), raw.len());
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Gatekeeper => {
            let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
            let gatekeeper = skyward_crypto::gatekeeper::Gatekeeper::open(&binder, &hardware)?;
            println!("the credential service answers for {}", gatekeeper.serves()?);
            match gatekeeper.version() {
                Ok(Some(version)) => println!("it serves version {version} of that interface"),
                Ok(None) => println!("it numbers its version in its own name"),
                Err(e) => println!("it will not say which version it serves: {e:#}"),
            }
            // A credential belonging to no one, which the secure world
            // cannot recognise and counts against nobody. What comes back
            // says whether the service takes a check at all, apart from
            // whether it recognises what it is given.
            match gatekeeper.verify(NOBODY, &[], &[]) {
                Ok(_) => println!("it took a credential belonging to no one"),
                Err(e) => println!("asked about a credential belonging to no one it says: {e:#}"),
            }
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Weaver => {
            let binder = skyward_binder::driver::Binder::open()?;
            let c = skyward_crypto::weaver::Weaver::open(&binder)?.config()?;
            println!("the weaver has {} slots, keeps keys of {} bytes and values of {} bytes", c.slots, c.key_bytes, c.value_bytes);
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Data { close: true, .. } => {
            wipe::release_data()?;
            println!("the data partition is closed");
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Data { credential, close: false } => {
            use skyward_crypto::data;
            skyward_crypto::environment::pin()?;
            let binder = skyward_binder::driver::Binder::open()?;
            let service = skyward_crypto::keymint::KeyMint::open(&binder)?;
            let opening = data::open_without_credential(&service)?;
            match &opening.mounted {
                Some(mounted) => {
                    if let Some(checked) = &mounted.checked {
                        print!("{}", checked.said);
                        println!("the check of {} ended with {}", mounted.mapped.display(), checked.status);
                    }
                    println!("{} holds {} at {}", mounted.mapped.display(), mounted.filesystem, data::MOUNT);
                }
                None => println!("{} is mounted already", data::MOUNT),
            }
            let mut closed = opening.closed;
            let asked = closed.iter().find(|c| !c.made_up).map(|c| c.user);
            if let (Some(credential), Some(user)) = (&credential, asked) {
                closed = data::unlock(&service, &closed, user, credential.as_bytes())?;
            }
            for c in &closed {
                match &c.why {
                    Some(why) => println!("user {}'s own files stay closed: {why}", c.user),
                    None => println!("user {}'s own files stay closed", c.user),
                }
            }
            if closed.is_empty() {
                println!("every user's own files can be read");
            }
            Ok(())
        }
        #[cfg(target_os = "android")]
        Cmd::Check { config } => check(&config),
        #[cfg(target_os = "android")]
        Cmd::Ui { config, font, mono, unlock, dump } => {
            let cfg = DeviceConfig::load(&config)?;
            let start = if unlock { skyward_ui::app::Screen::Decrypt } else { skyward_ui::app::Screen::Home };
            ui::run(&cfg, &font, &mono, bcb::Request::default(), start, dump.as_deref())
        }
    }
}

fn booted_slot() -> Result<skyward_vab::Slot> {
    skyward_vab::current_slot()?.context("not an A/B device")
}

/// The lines of a service's own error output that are kept, from its end.
#[cfg(any(target_os = "android", test))]
const SAID_LINES: usize = 12;

/// Everything [`report`] prints, gathered by the caller.
///
/// It is a plain value so that the words can be tried on a computer: the
/// gathering needs a phone, the report does not.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Default)]
struct Gathered {
    codename: String,
    /// The build the image was packed on, and the first of its recovery sum.
    packed_on: Option<(String, String)>,
    slot: Option<char>,
    /// The installed system's incremental, release and patch level.
    installed: Option<(String, String, String)>,
    /// Each table the phone keeps, and whether it is there.
    tables: Vec<(&'static str, bool)>,
    data: Option<skyward_block::fstab::Entry>,
    /// Whether the data partition is mounted now, so that its files can be
    /// read; a recovery that has not been given what it needs to unwrap the
    /// key leaves it closed.
    data_open: bool,
    /// Each user and whether their own files are open, which is a different
    /// question from the partition being open: the partition opens with the
    /// key the phone keeps for it, and a user's files open behind their
    /// credential, or behind the one the phone made up for them.
    users: Vec<(u32, bool)>,
    /// Whether the table was read but said nothing about /data.
    said_nothing_about_data: bool,
    data_error: Option<String>,
    key_dir: Option<String>,
    /// The files in the key's directory: their names and sizes, never what
    /// they hold.
    key_files: Option<Vec<(String, u64)>>,
    key_error: Option<String>,
    read_through: Option<String>,
    device_error: Option<String>,
    /// Each service of the key chain, and whether it has registered.
    services: Vec<(String, bool)>,
    update: Option<String>,
    update_error: Option<String>,
    /// Each kept error output, with the lines the report shows.
    logs: Vec<(String, Vec<String>)>,
}

/// The report, as the words a person reads and pastes back.
#[cfg(any(target_os = "android", test))]
fn report(g: &Gathered) -> String {
    let mut out = String::new();
    out.push_str(&format!("skyward {} for {}\n", skyward_info::VERSION, g.codename));
    if let Some((version, sum)) = &g.packed_on {
        out.push_str(&format!("packed on {version}, from the recovery partition {sum}…\n"));
    }
    out.push_str(&format!(
        "running from slot {}\n",
        g.slot.map_or("unknown".to_string(), |s| s.to_string())
    ));
    match &g.installed {
        Some((incremental, release, patch)) => {
            out.push_str(&format!("installed system {incremental} (Android {release}, patch {patch})\n"));
        }
        None => out.push_str("installed system: its build properties could not be read\n"),
    }

    out.push_str("\nthe phone's own table\n");
    for (file, there) in &g.tables {
        out.push_str(&format!("  {file}: {}\n", if *there { "there" } else { "not there" }));
    }
    match (&g.data, g.said_nothing_about_data, &g.data_error) {
        (Some(entry), _, _) => {
            out.push_str("  /data\n");
            out.push_str(&format!("    device: {}\n", entry.device));
            out.push_str(&format!("    filesystem: {}\n", entry.filesystem));
            for name in ["metadata_encryption", "keydirectory", "fileencryption"] {
                out.push_str(&format!("    {name}: {}\n", entry.extra(name).unwrap_or("(none)")));
            }
            // Without this the partition cannot be mapped at all, and the
            // reason names it, so it is worth saying which it is.
            if entry.extra("metadata_encryption").is_none() {
                out.push_str("    note: no metadata_encryption, so this table says the partition is not encrypted whole\n");
            }
        }
        (None, true, _) => out.push_str("  it says nothing about /data\n"),
        (None, false, Some(error)) => out.push_str(&format!("  it could not be read: {error}\n")),
        (None, false, None) => out.push_str("  (the table was not read)\n"),
    }
    // Where the key is kept and what the partition would be read through are
    // both read off that entry, so both are said whatever became of it: a line
    // that says a look was not made is what this report is for.
    match (&g.key_dir, &g.key_files, &g.key_error) {
        (Some(dir), Some(files), _) => {
            let mut named: Vec<String> = files.iter().map(|(name, len)| format!("{name} ({len} bytes)")).collect();
            named.sort();
            let said = if named.is_empty() { "empty".to_string() } else { named.join(", ") };
            out.push_str(&format!("  the key it keeps: {dir}: {said}\n"));
        }
        (Some(dir), None, Some(error)) => out.push_str(&format!("  the key it keeps: {dir} ({error})\n")),
        (_, _, Some(error)) => out.push_str(&format!("  the key it keeps: ({error})\n")),
        _ => out.push_str("  the key it keeps: (not looked at, the table names none)\n"),
    }
    match (&g.read_through, &g.device_error) {
        (Some(device), _) => out.push_str(&format!("  /data is read through: {device}\n")),
        (None, Some(error)) => out.push_str(&format!("  /data is read through: ({error})\n")),
        _ => out.push_str("  /data is read through: (not looked at, the table names none)\n"),
    }
    // Whether it is open is the whole point of the rest of this report: the
    // key it keeps and the services above are what stands between a closed
    // partition and a readable one.
    out.push_str(match g.data_open {
        true => "  the data partition: open, its files can be read\n",
        false => "  the data partition: closed, so nothing in it can be read yet\n",
    });
    // The partition opening and a user's own files opening are two different
    // things: the first needs the key the phone keeps for it, the second needs
    // that user's credential, or the one the phone made up for them.
    if !g.users.is_empty() {
        let said = g
            .users
            .iter()
            .map(|(id, open)| format!("{id} {}", if *open { "open" } else { "closed" }))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  the users' own files: {said}\n"));
    }

    out.push_str("\nthe services the key chain needs\n");
    if g.services.is_empty() {
        out.push_str("  (the configuration names none)\n");
    }
    for (name, registered) in &g.services {
        let state = if *registered { "registered" } else { "NOT registered" };
        out.push_str(&format!("  {state}: {name}\n"));
    }

    out.push('\n');
    match (&g.update, &g.update_error) {
        (Some(state), _) => out.push_str(&format!("an update in flight: {state}\n")),
        (None, Some(error)) => out.push_str(&format!("an update in flight: ({error})\n")),
        _ => {}
    }

    if !g.logs.is_empty() {
        out.push_str("\nwhat the services said\n");
        for (path, lines) in &g.logs {
            out.push_str(&format!("  {path} ({} lines)\n", lines.len()));
            for line in lines {
                out.push_str(&format!("    | {line}\n"));
            }
        }
    }
    out
}

/// Say what opening the data partition needs, and whether each part is in
/// place.
///
/// This is the report to paste back when the partition does not open. Every
/// look is made on its own and a look that cannot be made says so: a report
/// that stopped at the first gap would be the one thing it is not for. The
/// names of the keys are printed and never what they hold.
#[cfg(target_os = "android")]
fn check(config: &Path) -> Result<()> {
    let cfg = DeviceConfig::load(config)?;
    let mut g = Gathered {
        codename: cfg.device.codename.clone(),
        ..Gathered::default()
    };
    if let Some(stock) = &cfg.stock {
        let sum = stock.recovery.get(..16).unwrap_or(&stock.recovery).to_string();
        g.packed_on = Some((stock.version.clone(), sum));
    }
    let slot = skyward_vab::current_slot().ok().flatten();
    g.slot = slot.map(|s| s.letter().chars().next().unwrap_or('?'));
    g.installed = slot
        .map(skyward_info::installed_props)
        .transpose()
        .ok()
        .flatten()
        .map(|props| {
            let said = |name: &str| props.get(name).map_or("(unset)", String::as_str).to_string();
            (
                said("ro.build.version.incremental"),
                said("ro.build.version.release"),
                said("ro.build.version.security_patch"),
            )
        });
    g.tables = skyward_block::fstab::FILES.iter().map(|file| (*file, Path::new(file).exists())).collect();
    match skyward_block::fstab::read("/data") {
        Ok(entries) if entries.is_empty() => g.said_nothing_about_data = true,
        Ok(entries) => {
            let entry = entries.into_iter().next().expect("an entry");
            match skyward_crypto::data::key_dir(&entry) {
                Ok(dir) => {
                    g.key_dir = Some(dir.display().to_string());
                    match std::fs::read_dir(&dir) {
                        Ok(files) => {
                            g.key_files = Some(
                                files
                                    .flatten()
                                    .map(|e| {
                                        let name = e.file_name().to_string_lossy().into_owned();
                                        (name, e.metadata().map(|m| m.len()).unwrap_or(0))
                                    })
                                    .collect(),
                            );
                        }
                        Err(e) => g.key_error = Some(e.to_string()),
                    }
                }
                Err(e) => g.key_error = Some(format!("{e:#}")),
            }
            match skyward_block::fstab::device(&entry) {
                Ok(device) => g.read_through = Some(device),
                Err(e) => g.device_error = Some(format!("{e:#}")),
            }
            g.data = Some(entry);
        }
        Err(e) => g.data_error = Some(format!("{e:#}")),
    }
    g.services = cfg
        .services
        .iter()
        .filter_map(|s| s.ready_service.as_ref().map(|name| (name.clone(), services::registered(name))))
        .collect();
    g.data_open = skyward_block::mount::is_mount_point(std::path::Path::new("/data")).unwrap_or(false);
    // A user whose files vold has opened has folders under their own
    // directory; one whose credential nobody has given yet has an empty one.
    // Whether a user's own files are open is not a matter of the folder
    // having anything in it: it is whether the key that folder is encrypted
    // with is in the kernel, which is what the kernel itself is asked rather
    // than guessed from a listing. The owner's files are the ones under
    // /data/data; another user's are their own folder.
    g.users = std::fs::read_dir("/data/user")
        .map(|entries| {
            let mut users: Vec<(u32, bool)> = entries
                .flatten()
                .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
                .map(|id| {
                    let dir = if id == 0 { std::path::PathBuf::from("/data/data") } else { std::path::PathBuf::from(format!("/data/user/{id}")) };
                    let open = skyward_crypto::fscrypt::unlocked(&dir).unwrap_or(false);
                    (id, open)
                })
                .collect();
            users.sort_unstable();
            users
        })
        .unwrap_or_default();
    match skyward_vab::snapshot_state() {
        Ok(state) => g.update = Some(format!("{state:?}")),
        Err(e) => g.update_error = Some(format!("{e:#}")),
    }
    g.logs = what_the_services_said();
    print!("{}", report(&g));
    Ok(())
}

/// The lines of a kept error output that the report shows: the last
/// [`SAID_LINES`] of them, the blank ones left out.
#[cfg(any(target_os = "android", test))]
fn kept_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(SAID_LINES);
    lines.into_iter().skip(from).map(str::to_string).collect()
}

/// The error output each of the phone's own programs has kept, with its last
/// lines, so that a program that cannot start is read rather than guessed at.
#[cfg(target_os = "android")]
fn what_the_services_said() -> Vec<(String, Vec<String>)> {
    let dir = Path::new("/skyward/run/services");
    let mut logs: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "log")).collect(),
        Err(_) => return Vec::new(),
    };
    logs.sort();
    logs.into_iter()
        .filter_map(|log| {
            let text = std::fs::read_to_string(&log).ok()?;
            Some((log.display().to_string(), kept_lines(&text)))
        })
        .collect()
}

/// Make a device whose blocks come from user space and take it away again,
/// saying what the kernel made of it.
///
/// A compressed snapshot is read through such a device: the kernel sends each
/// request to the program listening at `/dev/dm-user/<name>`, and that
/// program — the phone's `snapuserd` — answers with the blocks. Whether this
/// kernel has the target at all, and what it makes of a table naming it, is
/// the first thing that has to be known before any of that can be relied on.
#[cfg(target_os = "android")]
fn dm_user(name: &str, sectors: u64, params: Option<&str>) -> Result<()> {
    anyhow::ensure!(!name.is_empty() && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'), "{name} is not a name a device may have");
    // The target is registered as "user"; "/dev/dm-user/<name>" is the misc
    // device the kernel makes for it, and where the requests go. Asked for
    // "dm-user" the kernel knows nothing, which is worth telling apart.
    match skyward_block::dm::target_version("user").context("ask the kernel for its user target")? {
        Some([major, minor, patch]) => println!("the kernel's target is \"user\" {major}.{minor}.{patch}"),
        None => println!("the kernel has no \"user\" target, so nothing can be served from user space"),
    }
    let target = match params {
        Some(given) => {
            println!("trying the parameters as given: {given}");
            skyward_block::dm::Target::Raw { sectors, kind: "user".into(), params: given.to_string() }
        }
        None => skyward_block::dm::Target::DmUser { sectors, name: name.to_string(), partitions: 1 },
    };
    let device = skyward_block::dm::create(name, std::slice::from_ref(&target), true).context("make a device served from user space")?;
    println!("made {} ({sectors} sectors)", device.display());
    // The kernel makes a misc device named after the target as it creates
    // this one, and the program that serves it listens there.
    let control = Path::new("/dev/dm-user").join(name);
    println!("{}: {}", control.display(), if control.exists() { "there" } else { "not there" });
    skyward_block::dm::remove(name).with_context(|| format!("take {name} away"))?;
    println!("took {name} away again");
    Ok(())
}

fn lp(super_dev: &Path, slot: Option<u32>, plan_grow: Option<&str>) -> Result<()> {
    let slot = match slot {
        Some(n) => n,
        None => booted_slot()?.number(),
    };
    let md = skyward_lp::read(super_dev, slot)?;
    let g = md.geometry;
    println!("slot {slot}: version {}.{}, flags {:#x}", md.major_version, md.minor_version, md.header_flags);
    println!(
        "geometry: {} metadata slots of {} bytes, logical block {} bytes",
        g.metadata_slot_count, g.metadata_max_size, g.logical_block_size
    );
    for d in &md.damaged {
        println!("damaged: {d}");
    }
    for d in &md.block_devices {
        println!(
            "device {}: first sector {}, {} bytes, alignment {} offset {}",
            d.partition_name, d.first_logical_sector, d.size, d.alignment, d.alignment_offset
        );
    }
    for gr in &md.groups {
        println!("group {}: max {} bytes", gr.name, gr.maximum_size);
    }
    for p in &md.partitions {
        println!(
            "partition {} group {} attributes {:#x}, {} bytes",
            p.name, md.groups[p.group].name, p.attributes, p.bytes()
        );
        let mut start = 0;
        for e in &p.extents {
            let end = start + e.sectors() - 1;
            match *e {
                skyward_lp::Extent::Linear { device, sector, .. } => {
                    println!("  {start} .. {end} linear {} {sector}", md.block_devices[device].partition_name)
                }
                skyward_lp::Extent::Zero { .. } => println!("  {start} .. {end} zero"),
            }
            start = end + 1;
        }
    }
    // Only a plan, and nothing written: a partition may be grown as long as
    // the room is there and no sector that is not its own is taken, which is
    // worth seeing on this phone's own super before anything is changed.
    if let Some(spec) = plan_grow {
        let (name, wanted) = spec.split_once('=').with_context(|| format!("{spec} is not name=bytes"))?;
        let wanted: u64 = wanted.trim().parse().with_context(|| format!("{wanted} is not a number of bytes"))?;
        let before = md.partition(name).with_context(|| format!("super has no partition {name}"))?.bytes();
        let mut grown = skyward_lp::grow_across(&[(md, name.to_string(), wanted)]).context("plan the growth")?;
        let md = grown.pop().expect("the one metadata asked for");
        let part = md.partition(name).cloned().with_context(|| format!("the plan has no {name}"))?;
        println!("plan: {name} {before} -> {} bytes", part.bytes());
        let mut start = 0;
        for e in &part.extents {
            let end = start + e.sectors() - 1;
            match *e {
                skyward_lp::Extent::Linear { device, sector, .. } => {
                    println!("  {start} .. {end} linear {} {sector}", md.block_devices[device].partition_name);
                }
                skyward_lp::Extent::Zero { .. } => println!("  {start} .. {end} zero"),
            }
            start = end + 1;
        }
        println!("nothing was written");
    }
    Ok(())
}

/// Load `names` from the module sources the configuration lists.
fn load_modules(cfg: &DeviceConfig, names: &[String]) -> Result<Vec<PathBuf>> {
    let dirs = cfg.modules.sources.iter().map(|s| skyward_modules::source_dir(s)).collect::<Result<Vec<_>>>()?;
    skyward_modules::load(&dirs, names)
}

/// Start the recovery's own logd where it needs libraries of the installed
/// system, make the phone's firmware reachable to its drivers, mount the
/// partitions its own programs and drivers use where it mounts them, load
/// the configured kernel modules, hand the files the phone's init hands
/// over as it boots to their accounts, and start the phone's own programs,
/// read from the partitions just mounted: on the 13 Ultra, the touchscreen
/// driver and the touch processing. Of the programs the configuration
/// names, only those this phone's build of the hardware runs are started.
#[cfg(target_os = "android")]
fn bring_up(cfg: &DeviceConfig, slot: skyward_vab::Slot) -> Result<()> {
    let nodes = skyward_sys::ueventd::apply()?;
    skyward_sys::kmsg(&format!("{nodes} device nodes given the owner their phone describes"))?;
    let props = skyward_info::installed_props(slot)?;
    adopt_installed_version(&props)?;
    // Without logd the phone's own programs still run, and only what they
    // log is lost, so why it did not start goes to the kernel log.
    if let Some(logd) = &cfg.logd {
        if let Err(e) = start_logd(logd) {
            skyward_sys::kmsg(&format!("logd not started: {e:#}"))?;
        }
    }
    let services: Vec<_> = cfg.services.iter().filter(|s| s.runs_on(&props)).cloned().collect();
    // The drivers ask for their firmware by name, and the kernel looks only
    // in folders of its own unless told to pass on what it does not find
    // there to ueventd, which looks in the recovery's firmware folders.
    std::fs::write(FIRMWARE_FALLBACK, "1").with_context(|| format!("write 1 to {FIRMWARE_FALLBACK}"))?;
    if let Some(firmware) = &cfg.firmware {
        mount_firmware(&firmware.partition)?;
    }
    for name in &cfg.partitions.mounted {
        let at = skyward_block::fstab::mount(name)?;
        skyward_sys::kmsg(&format!("{name} is mounted on {}", at.display()))?;
    }
    for path in load_modules(cfg, &cfg.modules.load)? {
        skyward_sys::kmsg(&format!("loaded {}", path.display()))?;
    }
    // The secure element's HAL asks the element for a vendor id over the
    // ADSP before it can open the TA: started any earlier it logs
    // "eseGetVendorId: Unknown eSE HW" and exits (YARP's init says so), and
    // with it the weaver that guards a lock-screen secret. The kernel's own
    // remoteproc probe loads the DSP's firmware and boots it while init is
    // still running, so wait for it to say it is running before any service
    // starts, and where it could not start it, ask the loader again - the
    // firmware it could not reach is on the modem partition, which is shown
    // at /firmware by the mount above and is the last folder the loader
    // looks in. Never write the SSR switch beside it: that takes the DSP
    // down through rproc_shutdown, which hangs the writer in the ADSP
    // driver's shutdown path until the DSP is gone.
    if let Err(e) = wait_for_adsp() {
        skyward_sys::kmsg(&format!("the ADSP is not running: {e:#}"))?;
    }
    // The motor driver is loaded by init before the recovery can mount its
    // partitions. Put its firmware where the firmware loader looks before
    // that first probe, and unbind it again if that probe already failed.
    if let Some(haptics) = &cfg.haptics {
        if let Err(e) = haptics::bring_up(haptics, &cfg.modules.sources) {
            skyward_sys::kmsg(&format!("the motor's driver is not brought up: {e:#}"))?;
        }
    }
    if !cfg.owners.is_empty() {
        let handed = hand_over(&cfg.owners)?;
        skyward_sys::kmsg(&format!("{handed} of the phone's own files handed to the accounts its init gives them"))?;
    }
    let hardware: Vec<String> = services.iter().filter_map(|s| s.ready_hardware_service.clone()).collect();
    if !hardware.is_empty() {
        secure::prepare_hardware_directory(&hardware)?;
        skyward_sys::kmsg("the directory for the phone's hardware services can run")?;
    }
    // Only a program that has to register with the service directory needs
    // the directory to know the phone describes it. The WLAN's supplicant
    // registers whenever the WLAN is turned on, and the directory can no
    // longer start again by then: the key service registers with it below,
    // and nothing would register that again.
    let mut registering: Vec<String> = services.iter().flat_map(|s| s.ready_service.iter().chain(&s.registers).cloned()).collect();
    registering.extend(cfg.wlan.iter().map(|w| w.service.clone()));
    let files: Vec<String> = services.iter().filter_map(|s| s.description.clone()).collect();
    if !registering.is_empty() || !files.is_empty() {
        // A description the phone's build does not carry, as a service the
        // configuration lists for another build of it, leaves the directory
        // as it is; the programs the directory is for still start, and only
        // the service whose description is missing goes without. Ending here
        // would take the key service and the boot control with it, and with
        // them the data partition and the slots.
        match secure::publish_descriptions(&registering, &files) {
            Ok(described) => skyward_sys::kmsg(&format!("{described} of the phone's own service descriptions published"))?,
            Err(e) => skyward_sys::kmsg(&format!("the phone's own service descriptions are not published: {e:#}"))?,
        }
    }
    let libraries = secure::libraries()?;
    if let Some(dir) = &libraries {
        skyward_sys::kmsg(&format!("the phone's own libraries are at {}", dir.display()))?;
    }
    // The phone's hardware services are built against the installed system's
    // bionic, which a recovery's own library folder leaves out. Without the
    // two it is missing, every one of them ends as soon as it starts.
    match secure::provide_system_libraries() {
        Ok(0) => {}
        Ok(copied) => skyward_sys::kmsg(&format!("{copied} libraries of the installed system are in place for its services"))?,
        Err(e) => skyward_sys::kmsg(&format!("the installed system's libraries are not in place: {e:#}"))?,
    }
    services::start(&services, libraries.as_deref())?;
    agree_on_the_notes_key(&registering)
}

/// How long a file handed over may take to be there once the modules are
/// loaded: a driver makes its files as it takes its device up, and ueventd
/// makes the device's node once the kernel has told it of the device,
/// either of which may come after the load of the module returns.
#[cfg(target_os = "android")]
const FILES_MADE: std::time::Duration = std::time::Duration::from_secs(5);

/// Give each file the phone's own init hands over as it boots to the
/// account it names, with the mode it gives, as its chown and chmod lines
/// do. The files are the ones the modules just loaded made, which the
/// phone's own programs open as those accounts; one not there yet is
/// waited for.
#[cfg(target_os = "android")]
fn hand_over(owners: &[skyward_config::Owners]) -> Result<usize> {
    use skyward_sys::ueventd;
    use std::os::unix::fs::PermissionsExt;
    let known = ueventd::accounts(&ueventd::PASSWD_FILES.iter().chain(&ueventd::GROUP_FILES).copied().collect::<Vec<_>>());
    let id = |name: &str| ueventd::account(&known, name).with_context(|| format!("the phone has no account named {name}"));
    let mut handed = 0;
    for run in owners {
        let (uid, gid) = (id(&run.user)?, id(&run.group)?);
        for path in &run.paths {
            skyward_sys::wait_file(Path::new(path), FILES_MADE).context("wait for a file the modules just loaded make")?;
            std::os::unix::fs::chown(path, Some(uid), Some(gid)).with_context(|| format!("give {path} to {}:{}", run.user, run.group))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(run.mode))
                .with_context(|| format!("give {path} the mode {:o}", run.mode))?;
            handed += 1;
        }
    }
    Ok(handed)
}

/// The folder the recovery's own programs load their libraries from.
#[cfg(target_os = "android")]
const RECOVERY_LIBRARIES: &str = "/system/lib64";

/// How long logd may take to say it is ready once started.
#[cfg(target_os = "android")]
const LOGD_READY: std::time::Duration = std::time::Duration::from_secs(5);

/// Give the recovery's own logd the libraries of the installed system it
/// loads, and start it at once rather than on init's next try, so that it
/// keeps what the phone's own programs log from their start. Each library
/// is copied beside the recovery's own under a name of its own and then
/// renamed into place, so a logd an earlier run of the recovery service
/// started keeps the copy it has open.
#[cfg(target_os = "android")]
fn start_logd(logd: &skyward_config::Logd) -> Result<()> {
    for library in &logd.libraries {
        let from = Path::new(library);
        let to = Path::new(RECOVERY_LIBRARIES).join(from.file_name().expect("the configuration names each library's file"));
        let mut part = to.clone().into_os_string();
        part.push(".part");
        std::fs::copy(from, &part).with_context(|| format!("copy {library} to {}", Path::new(&part).display()))?;
        std::fs::rename(&part, &to).with_context(|| format!("put {} in place", to.display()))?;
    }
    skyward_sys::set_prop("ctl.start", "logd")?;
    skyward_sys::wait_prop("logd.ready", "true", LOGD_READY)?;
    skyward_sys::kmsg(&format!("logd runs with {} libraries of the installed system", logd.libraries.len()))
}

/// Have the parts of the secure world that keep keys agree on the key they
/// sign their notes with, where the phone has any that do. Android's key
/// service has them agree as the phone starts; until they have, the
/// credential service cannot write a note the key service will take, and
/// the keys the phone guards behind a screen lock stay unusable.
#[cfg(target_os = "android")]
fn agree_on_the_notes_key(registered: &[String]) -> Result<()> {
    let interface = format!("{}/", skyward_crypto::sharedsecret::INTERFACE);
    let names: Vec<String> = registered.iter().filter(|n| n.starts_with(&interface)).cloned().collect();
    if names.is_empty() {
        return Ok(());
    }
    let binder = skyward_binder::driver::Binder::open()?;
    let agreed = skyward_crypto::sharedsecret::agree(&binder, &names)?;
    skyward_sys::kmsg(&format!("{agreed} of the secure world agreed on the key it signs its notes with"))
}

/// A slot of the credential service that belongs to no user, which the
/// phone numbers its own from nothing upwards.
#[cfg(target_os = "android")]
const NOBODY: u32 = u32::MAX;

/// The switch that has the kernel pass the firmware requests it cannot
/// answer itself on to ueventd.
#[cfg(target_os = "android")]
const FIRMWARE_FALLBACK: &str = "/proc/sys/kernel/firmware_config/force_sysfs_fallback";

/// Where the firmware partition is shown as well: the recovery's own
/// ueventd looks in the image folder inside it.
#[cfg(target_os = "android")]
const FIRMWARE_SHOWN: &str = "/firmware";

/// Where the kernel shows the processors it can boot, one directory each.
#[cfg(target_os = "android")]
const REMOTEPROC: &str = "/sys/class/remoteproc";

/// How long the DSP is given to say it is running before it is asked for
/// again, and after.
///
/// Short on purpose. The kernel starts the DSP while the modules load, so a
/// phone whose firmware it can read says running within a second.
#[cfg(target_os = "android")]
const ADSP_READY: std::time::Duration = std::time::Duration::from_secs(3);

/// How long the DSP is given once the loader has been asked again.
#[cfg(target_os = "android")]
const ADSP_RETRY: std::time::Duration = std::time::Duration::from_secs(10);

/// The switch that has the phone's ADSP loader read the DSP's firmware and
/// start it, which is what its own init writes as the modules come in.
#[cfg(target_os = "android")]
const ADSP_BOOT: &str = "/sys/kernel/boot_adsp/boot";

/// Have the DSP started, or say why it is not.
///
/// The kernel reads the DSP's firmware while the modules are loading, and its
/// search ends with `/firmware/image` - a folder the recovery makes by
/// showing the modem partition there. On a phone whose `/vendor/firmware`
/// carries only the first part of that firmware, the kernel takes what it
/// finds there and stops at the first segment that is not: its own log reads
/// "could not find firmware for adsp.b28 ... attempted /firmware/image/
/// adsp.b28, open failed", and the DSP stays offline for the whole start.
/// The rest of the firmware is on the modem partition. Nothing has to be
/// copied for it: `/firmware` is in place by now, so asking the loader for
/// another try is enough, and this is the switch its own init writes.
///
/// The switch is only written while the DSP is not running: written to one
/// that is, it asks for a restart instead, and the SSR switch beside it takes
/// the DSP down through rproc_shutdown, which blocks the writer in the ADSP
/// driver's shutdown path until the DSP is gone. A phone whose DSP will not
/// come up at all still shows its pages.
#[cfg(target_os = "android")]
fn wait_for_adsp() -> Result<()> {
    if let Some(adsp) = adsp()? {
        if waiting_for_running(&adsp, ADSP_READY)? {
            return skyward_sys::kmsg(&format!("{} runs, so the secure element's HAL can reach it", adsp.display()));
        }
        // Offline: the kernel's own attempt did not have `/firmware` to look
        // in. Ask now that it does. A phone without that switch is left as it
        // is, and so is one whose firmware is missing outright - the loader
        // says so in the kernel log and the moment below runs out.
        match std::fs::write(ADSP_BOOT, "1") {
            Ok(()) => skyward_sys::kmsg("the ADSP is offline; asked the loader again now that /firmware is in place")?,
            Err(e) => skyward_sys::kmsg(&format!("the ADSP is offline and cannot be asked again: {e}"))?,
        }
        if waiting_for_running(&adsp, ADSP_RETRY)? {
            return skyward_sys::kmsg(&format!("{} runs, so the secure element's HAL can reach it", adsp.display()));
        }
        return Err(anyhow!("it is not running, even after the loader was asked again"));
    }
    Err(anyhow!("the kernel shows no processor that is the DSP"))
}

/// Whether `adsp` says it is running within `within`.
#[cfg(target_os = "android")]
fn waiting_for_running(adsp: &Path, within: std::time::Duration) -> Result<bool> {
    let asked = std::time::Instant::now();
    loop {
        if adsp_running(adsp)? {
            return Ok(true);
        }
        if asked.elapsed() >= within {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// The processor the kernel names the ADSP, wherever its remoteproc number
/// puts it: the number is not the same on every phone of this family.
#[cfg(target_os = "android")]
fn adsp() -> Result<Option<PathBuf>> {
    let dirs = match std::fs::read_dir(REMOTEPROC) {
        Ok(dirs) => dirs,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("read {REMOTEPROC}")),
    };
    for dir in dirs {
        let dir = dir?;
        let name = match std::fs::read_to_string(dir.path().join("name")) {
            Ok(name) => name,
            Err(_) => continue,
        };
        if name.trim().contains("adsp") {
            return Ok(Some(dir.path()));
        }
    }
    Ok(None)
}

/// Whether the kernel says the processor is running.
#[cfg(target_os = "android")]
fn adsp_running(adsp: &Path) -> Result<bool> {
    let state = adsp.join("state");
    match std::fs::read_to_string(&state) {
        Ok(state) => Ok(state.trim() == "running"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("read {}", state.display())),
    }
}

/// Mount the partition holding the firmware images where and as the
/// phone's own table mounts it, which is where the loaders of the secure
/// world's apps look for theirs, and show it where the recovery's ueventd
/// looks for the rest. Both stay when the recovery service starts again.
#[cfg(target_os = "android")]
fn mount_firmware(partition: &str) -> Result<()> {
    let shown = Path::new(FIRMWARE_SHOWN);
    if skyward_block::mount::is_mount_point(shown)? {
        return Ok(());
    }
    let at = skyward_block::fstab::mount(partition)?;
    std::fs::create_dir_all(shown).with_context(|| format!("make {}", shown.display()))?;
    skyward_block::mount::bind(&at, shown)?;
    skyward_sys::kmsg(&format!("{partition} is mounted on {}, shown at {}", at.display(), shown.display()))
}

/// Set these to the installed system's values: the version and patch level
/// the secure world is told the running build has, the newer name for the
/// version, and the vendor partition's patch level. A recovery image carries
/// its own, older build's values, and the secure world then sees keys the
/// installed system made as made under a build it does not know, and refuses
/// them. The values are read from the partitions that own the names rather
/// than from the merged set, where a ported system's other maker's files could
/// have replaced them. An earlier run of the recovery service may have set one
/// already, and init sets such a property only once.
#[cfg(target_os = "android")]
fn adopt_installed_version(props: &std::collections::HashMap<String, String>) -> Result<()> {
    let values = skyward_info::keymint_props(props);
    let set = [
        ("ro.build.version.release", &values.release),
        ("ro.build.version.release_or_codename", &values.release_or_codename),
        ("ro.build.version.security_patch", &values.security_patch),
        ("ro.vendor.build.security_patch", &values.vendor_security_patch),
    ];
    for (name, (value, source)) in set {
        anyhow::ensure!(!value.is_empty(), "no value for {name}: the installed build and any skyward.keymint. override name none");
        if skyward_sys::prop(name).as_ref() != Some(value) {
            skyward_sys::set_prop(name, value)?;
        }
        skyward_sys::kmsg(&format!("{name}={value}, from {source}"))?;
    }
    Ok(())
}

/// The recovery service. Its own folder is made and its log kept there from
/// the start, and adb comes up first so a failure later on can be
/// inspected, file transfer joining it
/// once the screen starts; then the boot request in misc is read. A
/// request whose work
/// goes on stays in misc until the work is done; any other is cleared at
/// once, so that any reboot from here boots the system. Failures go to the
/// kernel log and still end the process, which init then starts again.
fn recovery() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let run = || -> Result<()> {
        // Open to the programs the recovery runs as other users, the
        // supplicant among them, whatever the umask.
        std::fs::create_dir_all(skyward_sys::RUN).with_context(|| format!("make {}", skyward_sys::RUN))?;
        std::fs::set_permissions(skyward_sys::RUN, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("open {} to the recovery's other users", skyward_sys::RUN))?;
        skyward_sys::keep_log(Path::new(skyward_sys::LOG))?;
        // A recovery that ended with a function of its own in the port took
        // the port with it; going through "none" takes that function off.
        if skyward_sys::prop("sys.usb.state").is_some_and(|s| s != "adb") {
            skyward_sys::usb::set("none", std::time::Duration::from_secs(5))?;
        }
        skyward_sys::set_prop("sys.usb.config", "adb")?;
        let misc = bcb::misc();
        let message = bcb::read(&misc)?;
        let request = message.request();
        if !request.work.lasts() {
            bcb::clear(&misc)?;
        }
        let cfg = DeviceConfig::load(Path::new("/").join(ramdisk::CONFIG))?;
        skyward_sys::kmsg(&format!(
            "{} {} recovery up, misc command {:?} args {:?}, asking for {:?}",
            cfg.device.codename,
            skyward_info::VERSION,
            message.command,
            message.args,
            request.work
        ))?;
        if !request.unknown.is_empty() {
            skyward_sys::kmsg(&format!("misc arguments this recovery does not know: {:?}", request.unknown))?;
        }
        if request.work == bcb::Work::Exit {
            return skyward_sys::power(if request.shutdown_after { Power::Off } else { Power::System });
        }
        // An update the system asked for starts counting its starts in misc,
        // as Android's own recovery counts them, so that one a restart cuts
        // off is taken up again as a retry; one that follows the kernel
        // stopping is not started at all.
        #[cfg(target_os = "android")]
        if let bcb::Work::Update(update) = &request.work {
            if update.retry == 0 && !ui::panicked() {
                bcb::write(&misc, &message.with_retry(1))?;
            }
        }
        // The kernel starts the clock from the RTC, days after 1970 on these
        // phones; the screen and the files written still work without the
        // real time, so a failure goes to the kernel log.
        #[cfg(target_os = "android")]
        match skyward_sys::clock::set_from_rtc() {
            Ok(ms) => skyward_sys::kmsg(&format!("clock set to {} ms from the RTC and persist", ms))?,
            Err(e) => skyward_sys::kmsg(&format!("clock not set: {e:#}"))?,
        }
        // The kernel starts every core at its top frequency, and keeps idle
        // ones from powering down, until the phone's own system boots and
        // changes that; left so, the phone drains its battery with the
        // screen off. The recovery works either way, so a failure goes to
        // the kernel log.
        #[cfg(target_os = "android")]
        match skyward_sys::cpu::run_as_system() {
            Ok(done) => skyward_sys::kmsg(&format!("the CPUs run as the system runs them: {}", done.join(", ")))?,
            Err(e) => skyward_sys::kmsg(&format!("the CPUs run as the kernel started them: {e:#}"))?,
        }
        #[cfg(target_os = "android")]
        {
            use skyward_ui::app::Screen;
            // fastbootd rewrites super, which it cannot do while the slot's
            // partitions are mapped and mounted, so fastboot runs without
            // them and without what runs from them. So does work on whole
            // partitions the system asks for, as in Android's own recovery:
            // erasing them, which may take super with them, and undoing
            // shared blocks, which writes to the slot's own.
            if !matches!(request.work, bcb::Work::Fastboot | bcb::Work::WipeAb { .. } | bcb::Work::UnshareBlocks) {
                prepare(&cfg, booted_slot()?)?;
                // A recovery flashed over the system leaves behind what ART
                // compiled for the system that was there. Clearing it once per
                // image makes the system compile its own again, which is what the
                // phone then runs: on the older code it re-verifies and recompiles
                // as it goes, and is slower and hotter for as long as that lasts.
                // The key service the data partition opens with is up by now,
                // since prepare brought it up.
                art::clear_once(open_data_for_art);
            }
            let root = Path::new("/");
            ui::run(&cfg, &root.join(ramdisk::FONT), &root.join(ramdisk::MONO), request, Screen::Home, None)
        }
        #[cfg(not(target_os = "android"))]
        {
            anyhow::bail!("the recovery service runs only on Android")
        }
    };
    run().inspect_err(|e| {
        let _ = skyward_sys::kmsg(&format!("recovery failed: {e:#}"));
    })
}

/// Open the data partition as far as it opens with nothing a user knows,
/// which is what clearing what ART compiled needs: none of it is a user's
/// own file. The key service has to answer, which `prepare` has by now
/// started; where it does not, the reason is what the caller says.
#[cfg(target_os = "android")]
fn open_data_for_art() -> Result<()> {
    use skyward_crypto::data;
    skyward_crypto::environment::pin()?;
    let binder = skyward_binder::driver::Binder::open()?;
    let service = skyward_crypto::keymint::KeyMint::open(&binder)?;
    data::open_without_credential(&service)?;
    Ok(())
}

/// Map and mount the slot's partitions from super, then bring up what runs
/// from them. Recovery has to stay usable when super cannot be mounted, as
/// after it is wiped, so the reason goes to the kernel log instead.
#[cfg(target_os = "android")]
fn prepare(cfg: &DeviceConfig, slot: skyward_vab::Slot) -> Result<()> {
    match skyward_vab::mount_partitions(slot) {
        Ok(mounts) => {
            for d in &mounts.damaged {
                skyward_sys::kmsg(&format!("super: {d}"))?;
            }
            for m in &mounts.partitions {
                skyward_sys::kmsg(&format!("mounted {} on {} ({})", m.partition, m.mount_point.display(), m.filesystem))?;
            }
            // Without them the screen still shows and the keys still work,
            // so this reason goes to the kernel log as well.
            if let Err(e) = bring_up(cfg, slot) {
                skyward_sys::kmsg(&format!("devices not brought up: {e:#}"))?;
            }
        }
        Err(e) => skyward_sys::kmsg(&format!("partitions not mounted: {e:#}"))?,
    }
    Ok(())
}

fn info(config: &str) -> Result<()> {
    let cfg = DeviceConfig::load(config)?;
    let i = skyward_info::collect(&cfg)?;
    println!("model         = {} ({})", i.model, i.codename);
    println!("soc           = {:?}", i.soc);
    println!("cpu           = {} cores, max {:?} kHz", i.cpu.cores, i.cpu.max_khz);
    println!("ram           = {} KiB", i.ram_kib);
    println!("panel         = {:?}", i.panel);
    println!("screen        = {:?} in", i.screen);
    println!("os_version    = {:?}", i.os_version);
    println!("android       = {:?}", i.android);
    println!("patch         = {:?}", i.security_patch);
    println!("kernel        = {}", i.kernel);
    println!("storage       = {} bytes, free {:?}", i.storage.total, i.storage.free);
    println!("battery       = {:?}", i.battery);
    println!("boot          = {:?}", i.boot);
    println!("recovery      = {}", skyward_info::VERSION);
    match i.slot {
        Some(s) => println!("current_slot  = {}", s.suffix()),
        None => println!("current_slot  = (not A/B)"),
    }
    let snap = i.snapshot;
    println!("snapshot      = {snap:?}");
    match snap {
        SnapshotState::None => {}
        SnapshotState::Unknown => println!("  -> /metadata is not mounted; dynamic-partition writes stay refused"),
        _ => println!("  -> dynamic-partition writes are refused while a snapshot is outstanding"),
    }
    let parts = skyward_block::list_by_name(skyward_block::BY_NAME)?;
    println!("partitions    = {} under {}", parts.len(), skyward_block::BY_NAME);
    Ok(())
}

fn identify(path: &Path) -> Result<()> {
    use skyward_install::package::{self, Package};
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    match package::identify(std::io::BufReader::new(file))? {
        Package::Update(u) => {
            println!("an A/B update to {}", u.version().unwrap_or("an unnamed version"));
            println!("payload.bin at byte {}, {} bytes", u.payload_offset, u.payload_size);
            for key in ["ota-type", "pre-device", "post-build", "post-timestamp", "post-security-patch-level", "pre-build"] {
                if let Some(v) = u.metadata.get(key) {
                    println!("{key:<26} {v}");
                }
            }
            if u.wipes() {
                println!("all user data is erased once it is in, after the user agrees");
            }
        }
        Package::Image(kind) => println!("{kind:?}, for {}", kind.partitions().join(" or ")),
        Package::Script => println!("a zip that installs itself with a script"),
        Package::System(c) => {
            println!("the system {} {} ({}), to install beside the installed one", c.manifest.name, c.manifest.version, c.manifest.id);
            println!("a boot image of {} bytes, a root image of {} bytes ({}), starting {}", c.boot, c.root, c.manifest.filesystem, c.manifest.init);
            println!("its root takes at least {} bytes", c.least());
            if c.init_boot {
                println!("it brings its own init_boot image");
            }
        }
        Package::Unknown => println!("nothing this recovery installs"),
    }
    Ok(())
}

fn slot() -> Result<()> {
    match skyward_vab::current_slot()? {
        Some(s) => println!("{}", s.suffix()),
        None => println!("(not A/B)"),
    }
    #[cfg(target_os = "android")]
    {
        use skyward_vab::boot::BootControl;
        let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
        let opened = BootControl::open(&binder, &hardware);
        match opened {
            Ok(control) => {
                let slots = control.slots()?;
                println!("next {}", slots.active.suffix());
                for slot in [skyward_vab::Slot::A, skyward_vab::Slot::B] {
                    let k = slot.number() as usize;
                    println!("{} bootable {} successful {}", slot.suffix(), slots.bootable[k], slots.successful[k]);
                }
                println!("update {:?}", control.merge_status()?);
            }
            Err(e) => println!("{e:#}"),
        }
    }
    Ok(())
}

fn update() -> Result<()> {
    let slot = booted_slot()?;
    println!("standing {:?}", skyward_vab::standing(slot)?);
    for kind in ["snapshot", "snapshot-merge", "user"] {
        println!("target {kind} {:?}", skyward_block::dm::target_version(kind)?);
    }
    let Some(snaps) = skyward_vab::snapshots::Snapshots::lock(false)? else {
        println!("no update was ever applied");
        return Ok(());
    };
    let u = snaps.update()?;
    println!("state {:?} phase {:?} failure {:?}", snaps.state()?, u.merge_phase, u.merge_failure_code);
    println!("snapuserd {} userspace {} allocated {} total {} metadata {}", u.using_snapuserd, u.userspace_snapshots, u.sectors_allocated, u.total_sectors, u.metadata_sectors);
    println!("from {:?}, {:?} for {}, built {:?}", snaps.source()?, snaps.side(slot)?, slot.suffix(), u.source_build_fingerprint);
    println!("needed {}", snaps.needed(slot)?);
    for name in snaps.list()? {
        let s = snaps.status(&name)?;
        println!(
            "{name}: {:?} device {} snapshot {} cow {} + {} allocated {} metadata {} snapuserd {} old {}",
            s.part_state()?,
            s.device_size,
            s.snapshot_size,
            s.cow_partition_size,
            s.cow_file_size,
            s.sectors_allocated,
            s.metadata_sectors,
            s.using_snapuserd,
            s.old_partition_size
        );
        for dev in [name.clone(), format!("{name}-base"), format!("{name}-cow"), format!("{name}-cow-img")] {
            if skyward_block::dm::status(&dev)?.is_some() {
                for line in skyward_block::dm::table_status(&dev)? {
                    println!("  {dev}: {} {}+{} {}", line.kind, line.start, line.sectors, line.text);
                }
            }
        }
    }
    Ok(())
}

fn modules(source: &str) -> Result<()> {
    let graph = skyward_modules::parse_dep(source)?;
    let order = graph.load_order(&[]);
    println!("{} modules in dependency order", order.len());
    for m in &order {
        println!("  {m}");
    }
    Ok(())
}

#[cfg(test)]
mod check_report_tests;

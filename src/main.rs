extern crate clap;
#[macro_use]
extern crate failure;
extern crate nix;
extern crate number_prefix;

use clap::{Arg, App, AppSettings};
use failure::Error;
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{self, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use number_prefix::{binary_prefix, Standalone, Prefixed};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::process::Command;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

const NS_PER_SEC: u32 = 1_000_000;

/// Format `duration` as seconds with a fractional component.
fn fmt_duration_as_secs<W: Write>(mut writer: W, duration: &Duration) -> Result<(), Error>
{
    write!(writer, "{}.{:03} s", duration.as_secs(), duration.subsec_nanos() / NS_PER_SEC)?;
    Ok(())
}

/// Read /proc/$pid/io and return the rchar and wchar values, which are the number of bytes
/// passed to `read` and `write`.
fn rchar_wchar(pid: u32) -> Result<(u64, u64), Error> {
    let path = format!("/proc/{}/io", pid);
    let mut f = File::open(&path)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;

    let mut rchar = None;
    let mut wchar = None;
    for line in s.lines() {
        let mut bits = line.split_whitespace();
        match bits.next() {
            Some("rchar:") => {
                rchar = bits.next().and_then(|r| u64::from_str(r).ok());
            }
            Some("wchar:") => {
                wchar = bits.next().and_then(|r| u64::from_str(r).ok());
            }
            _ => {}
        }
        if rchar.is_some() && wchar.is_some() {
            break;
        }
    }
    if let (Some(r), Some(w)) = (rchar, wchar) {
        Ok((r, w))
    } else {
        Err(format_err!("Missing rchar/wchar in proc/io!"))
    }
}

/// Format `bytes` as bytes per second over the Duration in `d`.
fn fmt_bytes_per<W: Write>(mut writer: W, bytes: u64, d: Duration) -> Result<(), Error> {
    let s = d.as_secs();
    let bps = if s > 0 {
        bytes / s
    } else {
        0
    };
    let bps = (bps as f64) + (d.subsec_nanos() / NS_PER_SEC) as f64;
    match binary_prefix(bps) {
        Standalone(bytes)   => write!(writer, "{} B/s", bytes)?,
        Prefixed(prefix, n) => write!(writer, "{:.0} {}B/s", n, prefix)?,
    }
    Ok(())
}

fn work() -> Result<(), Error> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .setting(AppSettings::TrailingVarArg)
        .arg(Arg::with_name("output")
             .short("o")
             .value_name("FILE")
             .help("Redirect output to FILE")
             .takes_value(true))
        .arg(Arg::with_name("cmd")
             .multiple(true)
             .use_delimiter(false)
             .help("Command to run")
        ).get_matches();
    let mut args = match matches.values_of_os("cmd") {
        Some(a) => a,
        None => return Err(format_err!("No command specified")),
    };
    let mut cmd = Command::new(args.next().unwrap());
    cmd.args(args);
    if let Some(out) = matches.value_of_os("output") {
        let f = OpenOptions::new().write(true).create(true).open(out)?;
        cmd.stdout(f);
    }
    let sleepy = Duration::from_millis(1000);
    let mut p = cmd.spawn()?;
    let pid = Pid::from_raw(p.id() as i32);
    let start = Instant::now();
    let mut last = (start, 0, 0);
    let mut stdout = io::stdout();
    loop {
        thread::sleep(sleepy);
        // Did the process exit?
        if let Some(_) = p.try_wait()? {
            return Ok(());
        }
        // Stop it.
        signal::kill(pid, Signal::SIGSTOP)?;
        // Wait for it to actually stop.
        match wait::waitpid(pid, Some(WaitPidFlag::WSTOPPED | WaitPidFlag::WUNTRACED))? {
            // This is what we expect.
            WaitStatus::Stopped(_, _) => {},
            // If it exited then we're done.
            WaitStatus::Exited(_, _) => return Ok(()),
            e @ _ => return Err(format_err!("Unexpected wait status: {:?}", e)),
        }
        let d = last.0.elapsed();
        let (rchar, wchar) = rchar_wchar(p.id())?;
        let new_rchar = rchar - last.1;
        let new_wchar = wchar - last.2;
        fmt_duration_as_secs(&mut stdout, &start.elapsed())?;
        print!(": ");
        fmt_bytes_per(&mut stdout, new_rchar, d)?;
        print!(" read, ");
        fmt_bytes_per(&mut stdout, new_wchar, d)?;
        println!(" write");
        // Resume it.
        last = (Instant::now(), rchar, wchar);
        signal::kill(pid, Signal::SIGCONT)?;
    }
}

fn main() {
    match work() {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    }
}

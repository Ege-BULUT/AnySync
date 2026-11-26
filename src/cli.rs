//! A transfer from the command line, on any of the three systems:
//!
//!   anysync --give <device id> [--only a,b]    ask that device to receive, then send
//!   anysync --accept [--only a,b]              approve the request waiting here, then do this side
//!
//! `--only` takes item ids or their starts (`anysync --scan` lists them); without it the receiver takes everything
//! that has a place on its system.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::bus::{self, Answer, Ask, Bus, Device, Request};
use crate::me::{self, Config};
use crate::os::Env;
use crate::session::{self, Offer, Picks};
use crate::wire::{self, Endpoint};

struct Ctx {
    bus: Arc<Bus>,
    cfg: Config,
    secret: [u8; 32],
}

fn connect() -> Result<Ctx, String> {
    let mut cfg = Config::load();
    let token = me::token().ok_or("Not signed in: run `gh auth login`, or set ANYSYNC_TOKEN.")?;
    if cfg.bus.is_empty() {
        cfg.bus = format!("{}/anysync-bus", bus::github_login(&token)?);
        bus::ensure_repo(&token, &cfg.bus)?;
        cfg.save();
    }
    let b = Arc::new(Bus::new(&cfg.bus, &token));
    let (secret, public) = me::keys();
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    let d = Device { id: cfg.id.clone(), name: cfg.name.clone(), look: cfg.look.clone(), key: bus::b64(&public), user, seen: bus::now(), version: env!("CARGO_PKG_VERSION").into(), ..Device::default() };
    b.put_json(&format!("devices/{}.json", d.id), &d, None, &format!("device {} ({})", d.name, d.id))?;
    cfg.registered = true;
    cfg.save();
    println!("{} #{} ({}) on {}", cfg.name, cfg.id, cfg.look.os, cfg.bus);
    Ok(Ctx { bus: b, cfg, secret })
}

fn device(b: &Bus, id: &str) -> Result<Device, String> {
    b.get_json::<Device>(&format!("devices/{id}.json"))?.map(|d| d.0).ok_or(format!("no device #{id} on the bus"))
}

/// Polls `f` every 5 s until it gives something; GitHub doesn't count the unchanged answers.
fn wait<T>(what: &str, limit: Duration, mut f: impl FnMut() -> Result<Option<T>, String>) -> Result<T, String> {
    println!("{what}…");
    let start = Instant::now();
    loop {
        if let Some(t) = f()? {
            return Ok(t);
        }
        if start.elapsed() > limit {
            return Err(format!("gave up: {what}"));
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

pub fn give(peer: &str, only: Vec<String>) -> Result<(), String> {
    let c = connect()?;
    let other = device(&c.bus, peer)?;
    let r = Request { ask: Ask::Send, from: c.cfg.id.clone(), to: other.id.clone(), at: bus::now(), answer: Answer::Pending, transfer: String::new() };
    c.bus.put_json(&r.path(), &r, None, &format!("Send request {} to {}", r.from, r.to))?;
    let r = wait(&format!("waiting for {} to approve", other.name), Duration::from_secs(3600), || {
        Ok(c.bus.get_json::<Request>(&r.path())?.map(|x| x.0).filter(|x| x.answer != Answer::Pending))
    })?;
    if r.answer != Answer::Accepted {
        return Err(format!("{} said {:?}", other.name, r.answer));
    }
    send(&c, &r.transfer, &other, &only)
}

pub fn accept(only: Vec<String>) -> Result<(), String> {
    let c = connect()?;
    let me = c.cfg.id.clone();
    let mut pending = None;
    for n in c.bus.list("requests")?.into_iter().filter(|n| n.ends_with(".json") && n.contains(&me)) {
        if let Some((r, sha)) = c.bus.get_json::<Request>(&format!("requests/{n}"))?
            && r.to == me && r.answer == Answer::Pending && bus::now().saturating_sub(r.at) < 86400
            && pending.as_ref().is_none_or(|(p, _): &(Request, String)| r.at > p.at)
        {
            pending = Some((r, sha));
        }
    }
    let (r, sha) = pending.ok_or("no request is waiting for this device")?;
    let other = device(&c.bus, if r.from == me { &r.to } else { &r.from })?;
    let names = [(me.clone(), c.cfg.name.clone()), (other.id.clone(), other.name.clone())].into_iter().collect();
    let r = session::approve(&c.bus, r, &sha, &names)?;
    println!("approved {:?} from {}: {}", r.ask, other.name, r.transfer);
    if r.sides().0 == me { send(&c, &r.transfer, &other, &only) } else { receive(&c, &r.transfer, &other, &only) }
}

fn send(c: &Ctx, t: &str, to: &Device, only: &[String]) -> Result<(), String> {
    let here = Env::here();
    let mut offer = session::offer(&here);
    if !only.is_empty() {
        offer.items.retain(|i| only.iter().any(|p| i.id == *p || i.id.starts_with(&format!("{p}/"))));
    }
    println!("offering {} items", offer.items.len());
    let key = to.key_bytes().ok_or("the other device has no key on the bus")?;
    session::put_sealed(&c.bus, &format!("{t}/offer.sealed"), &key, &offer, "offer")?;
    session::update_session(&c.bus, t, |s| s.state = "picking".into())?;
    let picks: Picks = wait(&format!("waiting for {} to pick", to.name), Duration::from_secs(3600), || session::get_sealed(&c.bus, &format!("{t}/picks.sealed"), &c.secret))?;
    let items: Vec<_> = offer.items.into_iter().filter(|i| picks.ids.contains(&i.id)).collect();
    println!("{} picked {} items", to.name, items.len());
    let (listener, port) = wire::listen().map_err(|e| e.to_string())?;
    let stream_key = crate::seal::random::<32>();
    let ep = Endpoint { key: stream_key.to_vec(), port, addrs: wire::addresses() };
    session::put_sealed(&c.bus, &format!("{t}/key.sealed"), &key, &ep, "stream key")?;
    session::update_session(&c.bus, t, |s| s.state = "sending".into())?;
    wire::serve(listener, stream_key, items, &here, |p| println!("sending {:.1} MB  {} items  {}", p.bytes as f64 / 1e6, p.items_done, p.current))?;
    session::update_session(&c.bus, t, |s| s.state = "done".into())?;
    println!("done");
    Ok(())
}

fn receive(c: &Ctx, t: &str, from: &Device, only: &[String]) -> Result<(), String> {
    let here = Env::here();
    let offer: Offer = wait(&format!("waiting for {} to offer", from.name), Duration::from_secs(3600), || session::get_sealed(&c.bus, &format!("{t}/offer.sealed"), &c.secret))?;
    let picks = session::picks(&offer, here.os, only);
    println!("taking {} of {} offered items (from {})", picks.ids.len(), offer.items.len(), offer.os);
    let key = from.key_bytes().ok_or("the other device has no key on the bus")?;
    session::put_sealed(&c.bus, &format!("{t}/picks.sealed"), &key, &picks, "picks")?;
    let ep: Endpoint = wait(&format!("waiting for {} to start sending", from.name), Duration::from_secs(3600), || session::get_sealed(&c.bus, &format!("{t}/key.sealed"), &c.secret))?;
    let dir = session::local_dir(&here, t);
    let staging = dir.join("staging");
    // A cut connection: ask again for what hasn't arrived, up to 20 times.
    for _ in 0..20 {
        let need: Vec<String> = picks.ids.iter().filter(|id| !staging.join(format!("{}.done", wire::safe(id))).exists()).cloned().collect();
        if need.is_empty() {
            break;
        }
        match wire::fetch(&ep, need, &staging, |id, _| println!("received {id}")) {
            Ok(_) => {}
            Err(e) => {
                println!("{e}; trying again");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
    let missing = picks.ids.iter().filter(|id| !staging.join(format!("{}.done", wire::safe(id))).exists()).count();
    // Tell the sender it can stop.
    let _ = wire::fetch(&ep, vec![], &staging, |_, _| {});
    let (placed, problems) = session::finish(&offer, &picks, &here, &from.name, &dir);
    println!("placed {placed} paths, {problems} problems, {missing} items missing; the brief is {}", dir.join("GOAL.md").display());
    Ok(())
}

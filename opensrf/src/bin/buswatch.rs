use chrono::{DateTime, Local};
use opensrf::bus;
use opensrf::conf;
use std::fmt;
use std::env;
use std::thread;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_WAIT_TIME: u64 = 60; // 1 minute

// Redis lists are deleted every time the last value in the list is
// popped.  If a list key persists for many minutes, it means the list
// is never fully drained, suggesting the backend responsible for
// popping values from the list is no longer alive OR is perpetually
// under excessive load.  Tell keys to delete themselves after
// this many seconds of being unable to drain the list.
const DEFAULT_KEY_EXPIRE_SECS: u64 = 1800; // 30 minutes

struct BusWatch {
    bus: bus::Bus,
    wait_time: u64,
    config: Arc<conf::Config>,
    ttl: u64,
    _start_time: DateTime<Local>,
}

impl fmt::Display for BusWatch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Buswatch {}", self.config.client().domain())
    }
}

impl BusWatch {
    pub fn new(config: Arc<conf::Config>) -> Self {
        let bus = match bus::Bus::new(config.client()) {
            Ok(b) => b,
            Err(e) => panic!("Cannot connect bus: {}", e),
        };

        let wait_time = DEFAULT_WAIT_TIME;

        BusWatch {
            bus,
            config,
            wait_time,
            ttl: DEFAULT_KEY_EXPIRE_SECS,
            _start_time: Local::now(),
        }
    }

    /// Returns true if the caller should start over with a new
    /// buswatcher to recover from a potentially temporary bus
    /// connection error.  False if this is a clean shutdown.
    pub fn watch(&mut self) -> bool {
        let mut obj = json::object! {};

        loop {
            thread::sleep(Duration::from_secs(self.wait_time));

            // Check all opensrf keys.
            let keys = match self.bus.keys("opensrf:*") {
                Ok(k) => k,
                Err(e) => {
                    log::error!("Error in keys() command: {e}");
                    return true;
                }
            };

            if keys.len() == 0 {
                continue;
            }

            obj["stats"] = json::JsonValue::new_object();

            for key in keys.iter() {
                match self.bus.llen(key) {
                    Ok(l) => {
                        // The list may have cleared in the time between the
                        // time we called keys() and llen().
                        if l > 0 {
                            obj["stats"][key]["count"] = json::from(l);
                            // Uncomment this chunk to see the next opensrf
                            // message in the queue for this key as JSON.
                            if let Ok(list) = self.bus.lrange(key, 0, 1) {
                                if let Some(s) = list.get(0) {
                                    obj["stats"][key]["next_value"] = json::from(s.as_str());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let err = format!("Error reading LLEN list={key} error={e}");
                        log::error!("{err}");
                        return true;
                    }
                }

                match self.bus.ttl(key) {
                    Ok(ttl) => {
                        obj["stats"][key]["ttl"] = json::from(ttl);
                        if ttl == -1 {
                            log::debug!("Setting TTL for stale key {key}");
                            if let Err(e) = self.bus.set_key_timeout(key, self.ttl) {
                                log::error!("Error with set_key_timeout: {e}");
                                return true;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error with ttl: {e}");
                    }
                }
            }

            obj["time"] = json::from(format!("{}", Local::now().format("%FT%T%z")));

            log::info!("{}", obj.dump());
        }
    }
}

fn main() {
    let conf = opensrf::init::init().unwrap();

    log::info!("Starting buswatch at {}", conf.client().domain());

    let mut watcher = BusWatch::new(conf.into_shared());

    if let Ok(v) = env::var("OSRF_BUSWATCH_TTL") {
        if let Ok(v2) = v.parse::<u64>() {
            watcher.ttl = v2;
        }
    }

    loop {
        if watcher.watch() {
            log::error!("Restarting watcher after fatal error");
        } else {
            break;
        }
    }

    log::info!("Watcher exiting");
}

// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use tracing::Level;
use tracing_subscriber::filter::{LevelFilter, Targets};

use std::{collections::HashMap, str::FromStr, sync::LazyLock};

/// Logging presets.
static PRESETS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from_iter([
        (
            "i2cp",
            "emissary::destination,emissary::i2cp=trace,emissary::tunnel::pool=off",
        ),
        (
            "sam",
            "emissary::sam,emissary::streaming,emissary::destination,yosemite=trace,emissary::tunnel::pool=off,emissary::client-tunnel=trace",
        ),
        (
            "transit",
            "emissary::tunnel::pool=off,emissary::tunnel::transit=trace,emissary::transport-manager=debug",
        ),
    ])
});

/// Parse a string of logging targets into [`Targets`].
///
/// INFO is enabled by default.
pub(super) fn parse_log_targets(log: Option<String>) -> Targets {
    let mut targets = Targets::new().with_target("", Level::INFO);
    let mut log_targets = Vec::<&str>::new();

    let Some(log) = log else {
        return targets;
    };

    for target in PRESETS.get(log.as_str()).unwrap_or(&log.as_str()).split(",") {
        let split = target.split('=').collect::<Vec<_>>();
        log_targets.push(split.first().expect("valid log target"));

        let Some(level) = split.get(1) else {
            continue;
        };

        targets = log_targets.into_iter().fold(targets, |targets, log_target| {
            targets.with_target(
                log_target,
                LevelFilter::from_str(level).expect("valid level filter"),
            )
        });
        log_targets = Vec::new();
    }

    targets
}

#[macro_export]
macro_rules! init_logger {
    ($log:expr) => {{
        use crate::logger::parse_log_targets;
        use std::sync::Arc;
        use tracing_subscriber::{fmt::time::ChronoLocal, prelude::*, reload};

        let targets = parse_log_targets($log);
        let (filter, handle) = reload::Layer::new(targets);

        #[cfg(feature = "i2pcontrol")]
        let log_ring = {
            let ring = Arc::new(crate::i2pcontrol::observability::LogRing::default());
            let layer = crate::i2pcontrol::observability::LogRing::shared_layer(Arc::clone(&ring));
            (Some(ring), Some(layer))
        };
        #[cfg(not(feature = "i2pcontrol"))]
        let log_ring = (None::<Arc<()>>, None::<()>);

        #[cfg(feature = "i2pcontrol")]
        let _ = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_timer(ChronoLocal::new(String::from("%H:%M:%S%.3f"))),
            )
            .with(filter)
            .with(log_ring.1.expect("I2PControl log layer is initialized"))
            .try_init();
        #[cfg(not(feature = "i2pcontrol"))]
        let _ = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_timer(ChronoLocal::new(String::from("%H:%M:%S%.3f"))),
            )
            .with(filter)
            .try_init();

        (handle, log_ring.0)
    }};
    ($log:expr, $handle:ident) => {{
        $handle.reload(crate::logger::parse_log_targets($log)).expect("to succeed");
    }};
}

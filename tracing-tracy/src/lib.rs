//! Collect [Tracy] profiles in tracing-enabled applications.
//!
//! Assuming the application is well instrumented, this should in practice be a very low effort way
//! to gain great amounts of insight into an application performance.
//!
//! Note, however that Tracy is ultimately a profiling, not an observability, tool. As thus, some
//! of tracing concepts cannot be represented well by Tracy. For instance, out-of-order span
//! entries and exits, are not supported, and neither are spans that are entered and exited on
//! different threads. This crate will attempt to mitigate the problems and retain trace validity
//! at the cost of potentially invalid data. When such a mitigation occurs, trace will contain a
//! message with a note about the problem.
//!
//! Some other caveats to keep in mind:
//!
//! * Only span entries and exits are recorded;
//! * Events show up as messages in Tracy, however Tracy can struggle with large numbers of
//! messages;
//! * Some additional functionality such as plotting and memory allocation profiling is only
//! available as part of the [tracy-client](tracy_client) crate.
//!
//! # Examples
//!
//! The most basic way to setup the tracy subscriber globally is as follows:
//!
//! ```rust
//! use tracing_subscriber::layer::SubscriberExt;
//!
//! tracing::subscriber::set_global_default(
//!     tracing_subscriber::registry()
//!         .with(tracing_tracy::TracyLayer::new()),
//! ).expect("set up the subscriber");
//! ```
//!
//! # Important note
//!
//! Unlike with many other subscriber implementations, simply depending on this crate is sufficient
//! for tracy to be enabled at program startup, even if [`TracyLayer`](TracyLayer) is not
//! registered as a subscriber. While not registering a `TracyLayer` will avoid Tracy from
//! collecting spans, it still broadcasts discovery packets to the local network and exposes the
//! data it collects in the background to that same network. Traces collected by Tracy may include
//! source and assembly code as well.
//!
//! As thus, you may want make sure to only enable the `tracing-tracy` crate conditionally, via the
//! `enable` feature flag provided by this crate.
//!
//! [Tracy]: https://github.com/wolfpld/tracy

use std::{fmt::Write, collections::VecDeque, cell::RefCell};
use tracing_core::{
    field::{Field, Visit},
    span::Id,
    Event, Subscriber,
};
use tracing_subscriber::fmt::format::{DefaultFields, FormatFields};
use tracing_subscriber::{
    fmt::FormattedFields,
    layer::{Context, Layer},
    registry,
};

use tracy_client::{Span, color_message, message, finish_continuous_frame};

thread_local! {
    /// A stack of spans currently active on the current thread.
    static TRACY_SPAN_STACK: RefCell<VecDeque<(Span, u64)>> =
        RefCell::new(VecDeque::with_capacity(16));
}

/// A tracing layer that collects data in Tracy profiling format.
#[derive(Clone)]
pub struct TracyLayer<F = DefaultFields> {
    format: F,
    stack_depth: u16,
}

impl TracyLayer {
    /// Create a new `TracyLayer`.
    ///
    /// Defaults to collecting stack traces.
    pub fn new() -> Self {
        Self { format: DefaultFields::new(), stack_depth: 64 }
    }

    /// Specify the maximum number of stack frames that will be collected.
    ///
    /// Specifying 0 frames will disable stack trace collection.
    pub fn with_stackdepth(mut self, stack_depth: u16) -> Self {
        self.stack_depth = stack_depth;
        self
    }
}

impl Default for TracyLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, F> Layer<S> for TracyLayer<F>
where
    S: Subscriber + for<'a> registry::LookupSpan<'a>,
    F: for<'writer> FormatFields<'writer> + 'static,
{
    fn on_enter(&self, id: &Id, ctx: Context<S>) {
        if let Some(span_data) = ctx.span(id) {
            let metadata = span_data.metadata();
            let file = metadata.file().unwrap_or("<error: not available>");
            let line = metadata.line().unwrap_or(0);
            let name = if let Some(fields) = span_data.extensions().get::<FormattedFields<F>>() {
                format!("{}: {}", metadata.name(), fields.fields.as_str())
            } else {
                metadata.name().to_string()
            };
            TRACY_SPAN_STACK.with(|s| {
                s.borrow_mut().push_back((
                    Span::new(&name, "", file, line, self.stack_depth),
                    id.into_u64()
                ));
            });
        }
    }

    fn on_exit(&self, id: &Id, _: Context<S>) {
        TRACY_SPAN_STACK.with(|s| {
            if let Some((span, span_id)) = s.borrow_mut().pop_back() {
                if id.into_u64() != span_id {
                    color_message(
                        "Tracing spans exited out of order! \
                        Trace may not be accurate for this span stack.",
                        0xFF000000,
                        self.stack_depth,
                    );
                }
                drop(span);
            } else {
                color_message(
                    "Exiting a tracing span, but got nothing on the tracy span stack!",
                    0xFF000000,
                    self.stack_depth,
                );
            }
        });
    }

    fn on_event(&self, event: &Event, _: Context<'_, S>) {
        let mut visitor = TracyEventFieldVisitor {
            dest: String::new(),
            first: true,
            frame_mark: false,
        };
        event.record(&mut visitor);
        if !visitor.first {
            let mut max_len = usize::from(u16::max_value()) - 1;
            if visitor.dest.len() >= max_len {
                while !visitor.dest.is_char_boundary(max_len) {
                    max_len -= 1;
                }
                message(&visitor.dest[..max_len], self.stack_depth);
                color_message(
                    "Message for the previous event was too long, truncated",
                    0xFF000000,
                    self.stack_depth,
                );
            } else {
                message(&visitor.dest, self.stack_depth);
            }
        }
        if visitor.frame_mark {
            finish_continuous_frame!();
        }
    }
}

struct TracyEventFieldVisitor {
    dest: String,
    frame_mark: bool,
    first: bool,
}

impl Visit for TracyEventFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        // FIXME: this is a very crude formatter, but we don’t have
        // an easy way to do anything better...
        if self.first {
            let _ = write!(&mut self.dest, "{} = {:?}", field.name(), value);
            self.first = false;
        } else {
            let _ = write!(&mut self.dest, ", {} = {:?}", field.name(), value);
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        match (value, field.name()) {
            (true, "tracy.frame_mark") => self.frame_mark = true,
            _ => self.record_debug(field, &value),
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
fn main() {
    tests::main();
}

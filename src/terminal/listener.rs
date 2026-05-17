use alacritty_terminal::event::{Event as AlacEvent, EventListener};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// Forwards alacritty events from the event-loop thread onto a futures channel
/// the gpui task awaits. `send_event` is called on the alacritty thread.
#[derive(Clone)]
pub struct SwrmListener {
    tx: UnboundedSender<AlacEvent>,
}

impl SwrmListener {
    pub fn pair() -> (Self, UnboundedReceiver<AlacEvent>) {
        let (tx, rx) = unbounded();
        (Self { tx }, rx)
    }
}

impl EventListener for SwrmListener {
    fn send_event(&self, event: AlacEvent) {
        let _ = self.tx.unbounded_send(event);
    }
}

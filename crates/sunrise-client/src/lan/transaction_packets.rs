//! Scoped packet capture for a tentative server action. Packets sent while a scope is active
//! wait in send order until the outermost transaction commits, then go through normal output;
//! rollback discards them without touching the connection's queue.
use super::{Body, PlayerSender};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

thread_local! {
    static SCOPES: RefCell<Vec<Packets>> = const { RefCell::new(Vec::new()) };
}

/// Encoded packets in send order, bound to their original connections.
#[derive(Default)]
pub struct Packets {
    bodies: Vec<(PlayerSender, Body)>,
}

impl Packets {
    /// Publish only after the outermost transaction has committed.
    pub fn publish(self) {
        for (sender, body) in self.bodies {
            sender.send_body(body);
        }
    }
}

/// A game-thread scope. Dropping it without `finish` discards tentative packets.
pub struct Scope {
    open: bool,
    _thread: PhantomData<Rc<()>>,
}
impl Scope {
    /// Begin capture before any tentative operation can send a packet.
    pub fn begin() -> Self {
        SCOPES.with(|s| s.borrow_mut().push(Packets::default()));
        Self {
            open: true,
            _thread: PhantomData,
        }
    }
    /// Transactions nested inside a capturing action also isolate their packets.
    pub fn nested() -> Option<Self> {
        active().then(Self::begin)
    }
    /// Merge a successful child in send order, or transfer the outer scope to the journal.
    pub fn finish(mut self) -> Option<Packets> {
        self.open = false;
        SCOPES.with(|s| {
            let mut scopes = s.borrow_mut();
            let packets = scopes.pop().expect("packet scope is current");
            match scopes.last_mut() {
                Some(parent) => {
                    parent.bodies.extend(packets.bodies);
                    None
                }
                None => Some(packets),
            }
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.open {
            SCOPES.with(|s| {
                s.borrow_mut().pop().expect("packet scope is current");
            });
        }
    }
}

pub(super) fn active() -> bool {
    SCOPES.with(|s| !s.borrow().is_empty())
}

pub(super) fn capture(sender: &PlayerSender, body: Body) -> Result<(), Body> {
    SCOPES.with(|s| {
        let mut scopes = s.borrow_mut();
        let Some(packets) = scopes.last_mut() else {
            return Err(body);
        };
        packets.bodies.push((sender.clone(), body));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan::{conn::Out, io_pool};
    use std::sync::{Arc, atomic::Ordering, mpsc};

    #[test]
    fn defers_nested_bodies_in_order_discards_on_abort_and_publishes_on_commit() {
        let (tx, rx) = mpsc::channel();
        let sender = PlayerSender::new(tx, Arc::default());
        let charge = io_pool::charge(1);

        let outer = Scope::begin();
        sender.send(vec![1]);
        let child = Scope::nested().unwrap();
        sender.send(vec![0]);
        assert_eq!(sender.queued().1, 0);
        assert!(rx.try_recv().is_err());
        drop(child);
        assert_eq!(sender.queued().1, 0);
        assert!(rx.try_recv().is_err());
        let child = Scope::nested().unwrap();
        sender.send(vec![2]);
        assert!(child.finish().is_none());
        sender.send(vec![3]);
        assert_eq!(sender.queued().1, 0);
        assert!(rx.try_recv().is_err());
        outer.finish().unwrap().publish();
        assert_eq!(sender.queued().1, 3 * charge);
        for expected in 1..=3 {
            assert!(matches!(rx.try_recv().unwrap(), Out::Packet(body) if body == [expected]));
        }
        sender.pending.store(0, Ordering::Relaxed); // The test channel has no network drain.

        let outer = Scope::begin();
        sender.send(vec![4]);
        let child = Scope::nested().unwrap();
        sender.send(vec![5]);
        assert!(child.finish().is_none());
        drop(outer);
        assert_eq!(sender.queued().1, 0);
        assert!(rx.try_recv().is_err());
    }
}
